package model

type Uploader interface {
	Upload(task *Task) error
}
