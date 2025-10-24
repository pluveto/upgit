package aliyunoss

import (
	"fmt"
	"path/filepath"
	"time"

	"github.com/pluveto/upgit/lib/model"
	"github.com/pluveto/upgit/lib/xapp"
	"github.com/pluveto/upgit/lib/xlog"
	"github.com/tu6ge/oss-go"
)

type OSSConfig struct {
	Endpoint        string `toml:"endpoint" mapstructure:"endpoint" validate:"nonzero"`
	AccessKeyId     string `toml:"access_key_id" mapstructure:"access_key_id" validate:"nonzero"`
	AccessKeySecret string `toml:"access_key_secret" mapstructure:"access_key_secret" validate:"nonzero"`
	BucketName      string `toml:"bucket_name" mapstructure:"bucket_name" validate:"nonzero"`
	Host            string `toml:"host" mapstructure:"host" validate:"nonzero"`
}

type OSSUploader struct {
	Config OSSConfig
}

func (u OSSUploader) Upload(t *model.Task) error {
	now := time.Now()
	name := filepath.Base(t.LocalPath)
	var targetPath string
	if len(t.TargetDir) > 0 {
		targetPath = t.TargetDir + "/" + name
	} else {
		targetPath = xapp.Rename(name, now)
	}
	rawUrl := u.buildUrl(targetPath)
	url := xapp.ReplaceUrl(rawUrl)
	xlog.GVerbose.Info("uploading #TASK_%d %s\n", t.TaskId, t.LocalPath)
	err := u.PutFile(t.LocalPath, targetPath)
	if err == nil {
		xlog.GVerbose.Info("successfully uploaded #TASK_%d %s => %s\n", t.TaskId, t.LocalPath, url)
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

func (u *OSSUploader) buildUrl(path string) string {
	return fmt.Sprintf("%s/%s", u.Config.Host, path)
}

func (u *OSSUploader) PutFile(localPath, targetPath string) (err error) {
	endpoint := u.Config.Endpoint

	cli, err := oss.New(u.Config.AccessKeyId, u.Config.AccessKeySecret, u.Config.BucketName, "cn-shanghai")
	if err != nil {
		return err
	}
	err = cli.Bucket.SetEndPointDomain(endpoint)
	if err != nil {
		return err
	}

	obj := oss.NewObject(targetPath)

	err = obj.FilePath(localPath).ContentType("").Upload(&cli)
	return
}
