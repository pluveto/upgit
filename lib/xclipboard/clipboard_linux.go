package xclipboard

import (
	"errors"
	"runtime"
)

func ReadClipboardImage() (buf []byte, err error) {
	return nil, errors.New("unsupported for your operation system")
}
