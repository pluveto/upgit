package main

import (
	"errors"
	"runtime"
)

func ReadClipboardImage() (buf []byte, err error) {
	if runtime.GOOS == "windows" {
		return Win32_ReadClipboardImage()
	}
	return nil, errors.New("unsupported for your operation system")
}
