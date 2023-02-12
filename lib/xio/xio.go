package xio

import (
	"os"
	"path/filepath"
)

func AppendToFile(filePath string, data []byte) {
	err := os.MkdirAll(filepath.Dir(filePath), 0755)
	panicErrWithoutLog(err)
	file, err := os.OpenFile(filePath, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0755)
	defer file.Close()
	panicErrWithoutLog(err)
	file.Write(data)
}
func panicErrWithoutLog(err error) {
	if err != nil {
		panic(err)
	}
}

func ReadFile(filePath string) ([]byte, error) {
	return os.ReadFile(filePath)
}

func WriteFile(filePath string, buf []byte) error {
	err := os.WriteFile(filePath, buf, 0755)
	return err
}
