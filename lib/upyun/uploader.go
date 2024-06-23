package upyun

import (
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/pluveto/upgit/lib/model"
	"github.com/pluveto/upgit/lib/xapp"
	"github.com/pluveto/upgit/lib/xlog"
)

type UpyunConfig struct {
	Host       string `toml:"host" mapstructure:"host"  validate:"nonzero"`
	BucketName string `toml:"bucket_name" mapstructure:"bucket_name" validate:"nonzero"`
	UserName   string `toml:"user_name" mapstructure:"user_name" validate:"nonzero"`
	PassWord   string `toml:"pass_word" mapstructure:"pass_word" validate:"nonzero"`
}

type UpyunUploader struct {
	Config UpyunConfig
}

var urlfmt = "https://{host}/{path}"

func (u UpyunUploader) Upload(t *model.Task) error {
	now := time.Now()
	name := filepath.Base(t.LocalPath)
	var targetPath string
	if len(t.TargetDir) > 0 {
		targetPath = t.TargetDir + "/" + name
	} else {
		targetPath = xapp.Rename(name, now)
	}
	rawUrl := u.buildUrl(urlfmt, targetPath)
	url := xapp.ReplaceUrl(rawUrl)
	xlog.GVerbose.Info("uploading #TASK_%d %s\n", t.TaskId, t.LocalPath)
	// var err error
	err := u.PutFile(t.LocalPath, targetPath)
	if err == nil {
		xlog.GVerbose.Info("sucessfully uploaded #TASK_%d %s => %s\n", t.TaskId, t.LocalPath, url)
		t.Status = model.TASK_FINISHED
		t.Url = url
		t.FinishTime = time.Now()
		t.RawUrl = rawUrl
	} else {
		xlog.GVerbose.Info("failed to upload #TASK_%d %s : %s\n", t.TaskId, t.LocalPath, err.Error())
		t.Status = model.TASK_FAILED
		t.FinishTime = time.Now()
	}
	return err
}

func (u *UpyunUploader) buildUrl(urlfmt, path string) string {
	r := strings.NewReplacer(
		"{host}", u.Config.Host,
		"{path}", path,
	)
	return r.Replace(urlfmt)
}

func (u *UpyunUploader) PutFile(localPath, targetPath string) (err error) {
	upyun := NewUpYun(u.Config.BucketName, u.Config.UserName, u.Config.PassWord)
	file, err := os.OpenFile(localPath, os.O_RDONLY, 0644)
	if err != nil {
		return err
	}
	err = upyun.WriteFile(targetPath, file, true)
	return
}
