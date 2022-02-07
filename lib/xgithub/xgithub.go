package xgithub

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"net/http"

	"github.com/pluveto/upgit/lib/xapp"
)

type List []struct {
	Name        string `json:"name"`
	Path        string `json:"path"`
	Sha         string `json:"sha"`
	Size        int    `json:"size"`
	URL         string `json:"url"`
	HTMLURL     string `json:"html_url"`
	GitURL      string `json:"git_url"`
	DownloadURL string `json:"download_url"`
	Type        string `json:"type"`
	Links       Links  `json:"_links"`
}
type Links struct {
	Self string `json:"self"`
	Git  string `json:"git"`
	HTML string `json:"html"`
}

func trimSlash(path string) string {
	if path[0] == '/' {
		path = path[1:]
	}
	if path[len(path)-1] == '/' {
		path = path[:len(path)-1]
	}
	return path
}

// ListFolder
// repo: pluveto/upgit
func ListFolder(repo string, path string) (List, error) {
	url := "https://api.github.com/repos/" + repo + "/contents/" + trimSlash(path)
	// logger.Trace("GET " + url)
	req, err := http.NewRequest(http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("User-Agent", xapp.UserAgent)
	req.Header.Set("Accept", "application/vnd.github.v3+json")
	req.Header.Set("Content-Type", "application/json")
	// req.Header.Set("Authorization", "token "+PAT)
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, err
	}
	body, err := ioutil.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}
	// logger.Trace("response body: " + string(body))
	if !(200 <= resp.StatusCode && resp.StatusCode < 300) {
		return nil, fmt.Errorf("%d %s", resp.StatusCode, body)
	}
	var list List
	err = json.Unmarshal(body, &list)
	if err != nil {
		return nil, err
	}
	return list, nil
}

func GetFile(repo string, branch string, path string) ([]byte, error) {
	url := "https://raw.githubusercontent.com/" + repo + "/" + branch + "/" + trimSlash(path)
	// logger.Trace("GET " + url)
	req, err := http.NewRequest(http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("User-Agent", xapp.UserAgent)
	req.Header.Set("Accept", "application/vnd.github.v3+json")
	req.Header.Set("Content-Type", "application/json")
	// req.Header.Set("Authorization", "token "+PAT)
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, err
	}
	bodyBuf, err := ioutil.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}
	// logger.Trace("response body: " + string(body))
	if !(200 <= resp.StatusCode && resp.StatusCode < 300) {
		return nil, fmt.Errorf("%d %s", resp.StatusCode, bodyBuf)
	}
	return bodyBuf, nil
}
