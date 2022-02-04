package main

import (
	"bytes"

	"encoding/base64"
	"fmt"
	"io/ioutil"
	"net/http"
	"path/filepath"
	"strings"
	"time"
)

type UploadOptions struct {
	LocalPath string
}

type GithubUploader struct {
	Config     GithubUploaderConfig
	OnUploaded func(result Result[*Task])
}

const kRawUrlFmt = "https://raw.githubusercontent.com/{username}/{repo}/{branch}/{path}"
const kApiFmt = "https://api.github.com/repos/{username}/{repo}/contents/{path}"

func (u GithubUploader) PutFile(message, path, name string) (err error) {
	dat, err := ioutil.ReadFile(path)
	if err != nil {
		return err
	}
	encoded := base64.StdEncoding.EncodeToString(dat)
	url := u.buildUrl(kApiFmt, name)
	GVerbose.Trace("PUT " + url)
	req, err := http.NewRequest(http.MethodPut, url, bytes.NewBufferString(
		`{
			"branch": "`+u.Config.Branch+`",
			"message": "`+message+`",
			"content": "`+encoded+`"
		}`))
	if err != nil {
		return err
	}
	req.Header.Set("User-Agent", "UPGIT/0.1")
	req.Header.Set("Accept", "application/vnd.github.v3+json")
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "token "+u.Config.PAT)
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return err
	}
	body, err := ioutil.ReadAll(resp.Body)
	if err != nil {
		return err
	}
	GVerbose.Trace("response body: " + string(body))
	if !(200 <= resp.StatusCode && resp.StatusCode < 300) {
		return fmt.Errorf("unexpected status code %d. response: %s", resp.StatusCode, string(body))
	}
	return nil
}

func (u GithubUploader) Upload(taskId int, localPath, targetDir string) (ret Result[*Task]) {
	now := time.Now()
	base := filepath.Base(localPath)
	// TODO: USE reference
	var targetPath string
	if len(targetDir) > 0 {
		targetPath = targetDir + "/" + base
	} else {
		targetPath = Rename(base, now)
	}
	rawUrl := u.buildUrl(kRawUrlFmt, targetPath)
	url := ReplaceUrl(rawUrl)
	GVerbose.Trace("uploading #TASK_%d %s\n", taskId, localPath)
	// var err error
	err := u.PutFile("upload "+base+" via upgit client", localPath, targetPath)
	if err == nil {
		GVerbose.Trace("sucessfully uploaded #TASK_%d %s => %s\n", taskId, localPath, url)
	} else {
		GVerbose.Trace("failed to upload #TASK_%d %s : %s\n", taskId, localPath, err.Error())
	}
	ret = Result[*Task]{
		err: err,
		value: &Task{
			Status:     TASK_FINISHED,
			TaskId:     taskId,
			LocalPath:  localPath,
			RawUrl:     rawUrl,
			Url:        url,
			FinishTime: now,
		}}
	return
}

func (u GithubUploader) buildUrl(urlfmt, path string) string {
	r := strings.NewReplacer(
		"{username}", u.Config.Username,
		"{repo}", u.Config.Repo,
		"{branch}", u.Config.Branch,
		"{path}", path,
	)
	return r.Replace(urlfmt)
}

// UploadAll will upload all given file to targetDir.
// If targetDir is not set, it will upload using rename rules.
func (u GithubUploader) UploadAll(localPaths []string, targetDir string) {
	for taskId, localPath := range localPaths {

		var ret Result[*Task]
		// ignore non-local path
		if strings.HasPrefix(localPath, "http") {
			ret = Result[*Task]{
				value: &Task{
					Ignored:    true,
					Status:     TASK_FINISHED,
					TaskId:     taskId,
					LocalPath:  localPath,
					RawUrl:     localPath,
					Url:        localPath,
					FinishTime: time.Now(),
				},
			}
		} else {
			ret = u.Upload(taskId, localPath, targetDir)
		}

		if ret.err == nil {
			GVerbose.TraceStruct(ret.value)
		}
		if nil != u.OnUploaded {
			u.OnUploaded(ret)
		}
	}
}
