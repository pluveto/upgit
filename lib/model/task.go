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
	Status     UploadStatus `toml:"status" mapstructure:"status"`
	TaskId     int          `toml:"task_id" mapstructure:"task_id"`
	LocalPath  string       `toml:"local_path" mapstructure:"local_path"`
	TargetDir  string       `toml:"target_dir" mapstructure:"target_dir"`
	TargetPath string       `toml:"target_path" mapstructure:"target_path"`
	Ignored    bool         `toml:"ignored" mapstructure:"ignored"`
	RawUrl     string       `toml:"raw_url" mapstructure:"raw_url"`
	Url        string       `toml:"url" mapstructure:"url"`
	CreateTime time.Time    `toml:"create_time" mapstructure:"create_time"`
	FinishTime time.Time    `toml:"finish_time" mapstructure:"finish_time"`
}
