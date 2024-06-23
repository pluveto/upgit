package uploaders

import (
	"bytes"

	"encoding/base64"
	"fmt"
	"io/ioutil"
	"net/http"
	"path/filepath"
	"strings"
	"time"

	"github.com/pluveto/upgit/lib/model"
	"github.com/pluveto/upgit/lib/xapp"
	"github.com/pluveto/upgit/lib/xlog"
)

type UploadOptions struct {
	LocalPath string
}

type GithubUploaderConfig struct {
	PAT      string `toml:"pat" validate:"nonzero"`
	Username string `toml:"username" validate:"nonzero"`
	Repo     string `toml:"repo" validate:"nonzero"`
	Branch   string `toml:"branch,omitempty"`
}
type GithubUploader struct {
	Config GithubUploaderConfig
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
	xlog.GVerbose.Trace("PUT " + url)
	req, err := http.NewRequest(http.MethodPut, url, bytes.NewBufferString(
		`{
			"branch": "`+u.Config.Branch+`",
			"message": "`+message+`",
			"content": "`+encoded+`"
		}`))
	if err != nil {
		return err
	}
	req.Header.Set("User-Agent", xapp.UserAgent)
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
	xlog.GVerbose.Trace("response body: " + string(body))
	if strings.Contains(string(body), "\\\"sha\\\" wasn't supplied.") {
		return nil
	}
	if !(200 <= resp.StatusCode && resp.StatusCode < 300) {
		return fmt.Errorf("unexpected status code %d. response: %s", resp.StatusCode, string(body))
	}
	return nil
}

func (u GithubUploader) Upload(t *model.Task) error {
	now := time.Now()
	base := filepath.Base(t.LocalPath)
	// TODO: USE reference
	var targetPath string
	if len(t.TargetDir) > 0 {
		targetPath = t.TargetDir + "/" + base
	} else {
		targetPath = xapp.Rename(base, now)
	}
	rawUrl := u.buildUrl(kRawUrlFmt, targetPath)
	url := xapp.ReplaceUrl(rawUrl)
	xlog.GVerbose.Info("uploading #TASK_%d %s\n", t.TaskId, t.LocalPath)
	// var err error
	err := u.PutFile("upload "+base+" via upgit client", t.LocalPath, targetPath)
	if err == nil {
		xlog.GVerbose.Info("sucessfully uploaded #TASK_%d %s => %s\n", t.TaskId, t.LocalPath, url)
	} else {
		xlog.GVerbose.Info("failed to upload #TASK_%d %s : %s\n", t.TaskId, t.LocalPath, err.Error())
	}
	t.Status = model.TASK_FINISHED
	t.Url = url
	t.FinishTime = time.Now()
	t.RawUrl = rawUrl
	return err
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
