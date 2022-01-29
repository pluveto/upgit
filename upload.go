package main

import (
	"bytes"
	"crypto/md5"
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

type UploadRet struct {
	TaskId    int
	LocalPath string
	RawUrl    string
	Url       string
	Time      time.Time
}

type GithubUploader struct {
	Options    Config
	OnUploaded func(result Result[UploadRet])
}

const kRawUrlFmt = "https://raw.githubusercontent.com/{username}/{repo}/{branch}/{path}"
const kApiFmt = "https://api.github.com/repos/{username}/{repo}/contents/{path}"

func (u GithubUploader) Rename(path string, time time.Time) (ret string) {
	base := filepath.Base(path)
	ext := filepath.Ext(path)
	r := strings.NewReplacer(
		"{year}", time.Format("2006"),
		"{month}", time.Format("01"),
		"{day}", time.Format("02"),
		"{unix_ts}", fmt.Sprint(time.Unix()),
		"{ext}", ext,
		"{file_name}", base,
		"{file_name_hash}", fmt.Sprintf("%x", md5.Sum([]byte(base))),
	)
	ret = r.Replace(u.Options.Rename)
	return
}
func (u GithubUploader) ReplaceUrl(path string) (ret string) {
	var rules []string
	for k, v := range u.Options.Replacements {
		rules = append(rules, k, v)
	}
	r := strings.NewReplacer(rules...)
	ret = r.Replace(path)
	return
}

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
			"branch": "`+u.Options.Branch+`",
			"message": "`+message+`",
			"content": "`+encoded+`"
		}`))
	if err != nil {
		return err
	}
	req.Header.Set("User-Agent", "UPGIT/0.1")
	req.Header.Set("Accept", "application/vnd.github.v3+json")
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "token "+u.Options.PAT)
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

func (u GithubUploader) Upload(taskId int, localPath string) (ret Result[UploadRet]) {
	now := time.Now()
	base := filepath.Base(localPath)
	renamed := u.Rename(base, now)
	rawUrl := u.buildUrl(kRawUrlFmt, renamed)
	url := u.ReplaceUrl(rawUrl)
	GVerbose.Trace("uploading #TASK_%d %s\n", taskId, localPath)
	err := u.PutFile("upload "+base+" via upgit client", localPath, renamed)
	if err == nil {
		GVerbose.Trace("sucessfully uploaded #TASK_%d %s => %s\n", taskId, localPath, url)
	} else {
		GVerbose.Trace("failed to upload #TASK_%d %s : %s\n", taskId, localPath, err.Error())
	}
	ret = Result[UploadRet]{
		err: err,
		value: UploadRet{
			TaskId:    taskId,
			LocalPath: localPath,
			RawUrl:    rawUrl,
			Url:       url,
			Time:      now,
		}}
	return
}

func (u GithubUploader) buildUrl(urlfmt, path string) string {
	r := strings.NewReplacer(
		"{username}", u.Options.Username,
		"{repo}", u.Options.Repo,
		"{branch}", u.Options.Branch,
		"{path}", path,
	)
	return r.Replace(urlfmt)
}

func (u GithubUploader) UploadAll(localPaths []string) {
	for taskId, localPath := range localPaths {
		ret := u.Upload(taskId, localPath)
		if ret.err == nil {
			GVerbose.TraceStruct(ret.value)
		}
		if nil != u.OnUploaded {
			u.OnUploaded(ret)
		}
	}
}
