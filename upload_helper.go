package main

import (
	"crypto/md5"
	"fmt"
	"path/filepath"
	"strings"
	"time"

	"github.com/pluveto/upgit/lib/xpath"
)

type UploadStatus string

const (
	TASK_CREATED  UploadStatus = "created"
	TASK_FINISHED              = "ok"
	TASK_PAUSED                = "paused"
	TASK_FAILED                = "failed"
)

type Task struct {
	Status     UploadStatus
	TaskId     int
	LocalPath  string
	TargetDir  string
	TargetPath string
	Ignored    bool
	RawUrl     string
	Url        string
	FinishTime time.Time
}

func Rename(path string, time time.Time) (ret string) {

	base := xpath.Basename(path)
	ext := filepath.Ext(path)
	md5HashStr := fmt.Sprintf("%x", md5.Sum([]byte(base)))
	r := strings.NewReplacer(
		"{year}", time.Format("2006"),
		"{month}", time.Format("01"),
		"{day}", time.Format("02"),
		"{unixts}", fmt.Sprint(time.Unix()),
		"{unixtsms}", fmt.Sprint(time.UnixMicro()),
		"{ext}", ext,
		"{fullname}", base+ext,
		"{filename}", base,
		"{filenamehash}", md5HashStr,
		"{fnamehash}", md5HashStr,
		"{fnamehash4}", md5HashStr[:4],
		"{fnamehash8}", md5HashStr[:8],
	)
	ret = r.Replace(cfg.Rename)
	return
}
func ReplaceUrl(path string) (ret string) {
	var rules []string
	for k, v := range cfg.Replacements {
		rules = append(rules, k, v)
	}
	r := strings.NewReplacer(rules...)
	ret = r.Replace(path)
	return
}
