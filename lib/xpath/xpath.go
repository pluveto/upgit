package xpath

import (
	"os"
	"path/filepath"
	"strings"
)

func Basename(path string) string {
	if pos := strings.LastIndexByte(path, '.'); pos != -1 {
		return filepath.Base(path[:pos])
	}
	return filepath.Base(path)

}

var ApplicationPath string

func GetApplicationPath() (path string, err error) {
	if ApplicationPath != "" {
		return ApplicationPath, nil
	}
	exec, err := os.Executable()
	if err != nil {
		return
	}
	path = filepath.Dir(exec)
	return
}
