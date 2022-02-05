package xnetwork

import (
	"errors"
	"io"
	"net/http"
	"os"
	"path/filepath"
)

func DownloadFileToFolder(url string, dir string) (err error) {
	os.MkdirAll(dir, 0755)
	out, err := os.Create(filepath.Join(dir, filepath.Base(url)))
	if err != nil {
		return
	}
	defer out.Close()
	err = DownloadFile(url, out)
	return
}

func DownloadFile(url string, out *os.File) (err error) {
	resp, err := http.Get(url)
	if err != nil {
		return
	}
	// check statuscode
	if resp.StatusCode != http.StatusOK {
		err = errors.New("unexpected statuscode: " + resp.Status)
		return
	}
	defer resp.Body.Close()
	_, err = io.Copy(out, resp.Body)
	return
}
