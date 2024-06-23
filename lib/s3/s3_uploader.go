package s3

import (
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/aws/aws-sdk-go/aws"
	"github.com/aws/aws-sdk-go/aws/credentials"
	"github.com/aws/aws-sdk-go/aws/session"
	"github.com/aws/aws-sdk-go/service/s3"
	"github.com/pluveto/upgit/lib/model"
	"github.com/pluveto/upgit/lib/xapp"
	"github.com/pluveto/upgit/lib/xlog"
)

type S3Config struct {
	Region     string `toml:"region" mapstructure:"region" validate:"nonzero"`
	BucketName string `toml:"bucket_name" mapstructure:"bucket_name" validate:"nonzero"`
	AccessKey  string `toml:"access_key" mapstructure:"access_key" validate:"nonzero"`
	SecretKey  string `toml:"secret_key" mapstructure:"secret_key" validate:"nonzero"`
	Endpoint   string `toml:"endpoint" mapstructure:"endpoint" validate:"nonzero"`
}

type S3Uploader struct {
	Config   S3Config
	s3Client *s3.S3
}

var urlfmt = "https://{bucket}.{endpoint}/{path}"

func (u S3Uploader) Upload(t *model.Task) error {
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

func (u *S3Uploader) buildUrl(urlfmt, path string) string {
	r := strings.NewReplacer(
		"{bucket}", u.Config.BucketName,
		"{endpoint}", u.Config.Endpoint,
		"{path}", path,
	)
	return r.Replace(urlfmt)
}

func (u *S3Uploader) PutFile(localPath, targetPath string) error {
	file, err := os.Open(localPath)
	if err != nil {
		return err
	}
	defer file.Close()

	_, err = u.s3Client.PutObject(&s3.PutObjectInput{
		Bucket: aws.String(u.Config.BucketName),
		Key:    aws.String(targetPath),
		Body:   file,
	})
	return err
}

func NewS3Uploader(config S3Config) (*S3Uploader, error) {
	sess, err := session.NewSession(&aws.Config{
		Region:           aws.String(config.Region),
		Endpoint:         aws.String(config.Endpoint),
		Credentials:      credentials.NewStaticCredentials(config.AccessKey, config.SecretKey, ""),
		S3ForcePathStyle: aws.Bool(true), // Required for some S3-compatible services
	})
	if err != nil {
		return nil, err
	}

	return &S3Uploader{
		Config:   config,
		s3Client: s3.New(sess),
	}, nil
}
