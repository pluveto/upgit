package model

import "time"

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
