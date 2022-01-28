package main

import (
	"os"
	"path/filepath"
)

func GetApplicationPath() (path string, err error) {
	exec, err := os.Executable()
	if err != nil {
		return
	}
	path = filepath.Dir(exec)
	return
}
