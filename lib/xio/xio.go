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
	file, err := os.Open(filePath)
	defer file.Close()
	if err != nil {
		return nil, err
	}
	fileInfo, err := file.Stat()
	if err != nil {
		return nil, err
	}
	var size = fileInfo.Size()
	bytes := make([]byte, size)
	buffer := make([]byte, 1024)
	for {
		c, err := file.Read(buffer)
		if err != nil {
			break
		}
		bytes = append(bytes, buffer[:c]...)
	}
	return bytes, nil
}

func WriteFile(filePath string, buf []byte) error {
	err := os.WriteFile(filePath, buf, 0755)
	return err
}
