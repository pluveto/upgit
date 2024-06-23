package qcloudcos

import (
	"bytes"
	"crypto/md5"
	"encoding/base64"
	"fmt"
	"io/ioutil"
	"mime"
	"net/http"
	"path/filepath"
	"strings"
	"time"

	"github.com/pluveto/upgit/lib/model"
	"github.com/pluveto/upgit/lib/xapp"
	"github.com/pluveto/upgit/lib/xlog"
	"github.com/pluveto/upgit/lib/xstrings"
)

type COSConfig struct {
	Host      string `toml:"host" mapstructure:"host"  validate:"nonzero"`
	SecretID  string `toml:"secret_id"   mapstructure:"secret_id"   validate:"nonzero"`
	SecretKey string `toml:"secret_key"  mapstructure:"secret_key"  validate:"nonzero"`
}

type COSUploader struct {
	Config COSConfig
}

var urlfmt = "https://{host}/{path}"

func (u COSUploader) Upload(t *model.Task) error {
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

func (u *COSUploader) buildUrl(urlfmt, path string) string {
	r := strings.NewReplacer(
		// <BucketName-APPID>.cos.<Region>.myqcloud.com
		"{host}", u.Config.Host,
		"{path}", path,
	)
	return r.Replace(urlfmt)
}

func (u *COSUploader) PutFile(localPath, targetPath string) (err error) {
	// prepare body

	// create request
	url := u.buildUrl(urlfmt, targetPath)
	xlog.GVerbose.Trace("PUT %s", url)
	req, err := http.NewRequest("PUT", url, nil)
	if err != nil {
		return err
	}
	// set header
	data, err := ioutil.ReadFile(localPath)
	if err != nil {
		return err
	}
	mimeType := mime.TypeByExtension(filepath.Ext(localPath))
	req.Host = u.Config.Host
	req.Header.Set("Date", time.Now().UTC().Format(http.TimeFormat))
	req.Header.Set("Content-MD5", base64.StdEncoding.EncodeToString(calMD5Digest(data)))
	req.Header.Set("Content-Type", xstrings.ValueOrDefault(mimeType, "application/octet-stream"))
	req.Header.Set("User-Agent", xapp.UserAgent)
	// set body
	req.Body = ioutil.NopCloser(bytes.NewBuffer(data))
	// send request
	resp, err := (&http.Client{Transport: &AuthorizationTransport{SecretID: u.Config.SecretID, SecretKey: u.Config.SecretKey}}).Do(req)

	xlog.GVerbose.Trace("request header:")
	xlog.GVerbose.TraceStruct(req.Header)

	if err != nil {
		return err
	}
	body, err := ioutil.ReadAll(resp.Body)
	if err != nil {
		return err
	}
	// check status code
	if resp.StatusCode != 200 {
		return fmt.Errorf("status code: %d, resp body: %s", resp.StatusCode, string(body))
	}
	return
}
func calMD5Digest(msg []byte) []byte {
	m := md5.New()
	m.Write(msg)
	return m.Sum(nil)
}
