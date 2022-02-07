package model

import "github.com/pluveto/upgit/lib/result"

type Uploader interface {
	Upload(task *Task) error
	SetCallback(func(result.Result[*Task]))
	GetCallback() func(result.Result[*Task])
}
