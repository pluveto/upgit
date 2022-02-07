package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"io/ioutil"
	"mime/multipart"
	"net/http"
	"path/filepath"
	"reflect"
	"strings"
	"time"

	"github.com/pluveto/upgit/lib/xmap"
	"github.com/pluveto/upgit/lib/xpath"
	"github.com/pluveto/upgit/lib/xstrings"
)

type SimpleHttpUploader struct {
	Config     map[string]interface{}
	Definition map[string]interface{}
	OnUploaded func(Result[*Task])
}

func (u SimpleHttpUploader) UploadAll(localPaths []string, targetDir string) {
	for taskId, localPath := range localPaths {

		var ret Result[*Task]
		task := Task{
			Status:     TASK_CREATED,
			TaskId:     taskId,
			LocalPath:  localPath,
			TargetDir:  targetDir,
			RawUrl:     localPath,
			Url:        localPath,
			FinishTime: time.Now(),
		}
		// ignore non-local path
		if strings.HasPrefix(localPath, "http") {
			task.Ignored = true
			task.Status = TASK_FINISHED
			ret = Result[*Task]{
				value: &task,
			}
		} else {
			ret = u.Upload(&task)
		}

		if ret.err == nil {
			GVerbose.TraceStruct(ret.value)
		}
		if nil != u.OnUploaded {
			u.OnUploaded(ret)
		}
	}
}

func MapGetOrDefault(m map[string]string, key, def string) string {
	if v, ok := m[key]; ok {
		return v
	}
	return def
}

func (u SimpleHttpUploader) replaceRequest(s string, data map[string]string) (ret string) {
	s = RemoveFmtUnderscore(s)
	r := strings.NewReplacer(
		"{remotepath}", MapGetOrDefault(data, "remote_path", ""),
	)
	return r.Replace(s)
}

func panicOnNilOrValue[T any](i interface{}, msg string) T {
	if nil == i {
		panic("value is nil: " + msg)
	}
	return i.(T)
}

func (u SimpleHttpUploader) replaceConfigPlaceholder(data map[string]interface{}, task *Task) {
	for k, v_ := range data {
		if reflect.TypeOf(v_).Kind() != reflect.String {
			continue
		}
		v := v_.(string)
		delim := []string{"$(", ")"}

		replacer := func(key string) *string {
			var ret string
			parentKey, subKey, found := strings.Cut(key, ".")
			if !found {
				return nil
			}
			if parentKey == "ext_config" {
				if v, ok := u.Config[subKey]; ok {
					ret = v.(string)
					return &ret
				}
			} else if parentKey == "config" {
				ret = GetValueByConfigTag(&cfg, subKey).(string)
				return &ret
			} else if parentKey == "option" {
				ret = GetValueByConfigTag(&opt, subKey).(string)
				return &ret
			} else if parentKey == "task" {
				ret = GetValueByConfigTag(task, subKey).(string)
				return &ret
			}
			return nil
		}
		ret := xstrings.VariableReplaceFunc(v, delim[0], delim[1], replacer)
		if nil != ret {
			data[k] = *ret
		}
	}
}

func GetValueByConfigTag(data interface{}, key string) (ret interface{}) {
	t := reflect.TypeOf(data)
	n := t.NumField()
	for i := 0; i < n; i++ {
		f := t.Field(i)
		if f.Tag.Get("json") == key || f.Tag.Get("yaml") == key || f.Tag.Get("toml") == key {
			return reflect.ValueOf(data).Field(i).Interface()
		}
	}
	return nil
}

func (u SimpleHttpUploader) UploadFile(task *Task) (rawUrl string, err error) {
	tplData := map[string]string{
		"remote_path": task.TargetPath,
	}
	method := FromGoRet[string](xmap.GetDeep[string](u.Definition, "http.request.method")).ValueOrExit()
	url := FromGoRet[string](xmap.GetDeep[string](u.Definition, "http.request.url")).ValueOrExit()
	GVerbose.Info("Method: %s, URL: %s", method, url)

	//  == Prepare header ==
	headers := FromGoRet[map[string]interface{}](xmap.GetDeep[map[string]interface{}](u.Definition, "http.request.headers")).ValueOrExit()
	u.replaceConfigPlaceholder(headers, task)
	GVerbose.Trace("unformatted headers:")
	GVerbose.TraceStruct(headers)
	headerCache := make(http.Header)
	for k, v := range headers {
		headerCache.Set(k, u.replaceRequest(v.(string), tplData))
	}
	if headerCache.Get("Content-Type") == "" {
		headerCache.Set("Content-Type", "application/octet-stream")
	}
	GVerbose.Trace("formatted headers:")
	GVerbose.TraceStruct(map[string][]string(headerCache))
	// upload file according to content-type
	contentType := headerCache.Get("Content-Type")

	// == Prepare body ==
	var body io.ReadCloser
	if contentType == "application/octet-stream" {
		body = ioutil.NopCloser(bytes.NewReader(FromGoRet[[]byte](ioutil.ReadFile(task.LocalPath)).ValueOrExit()))
	} else if contentType == "multipart/form-data" {
		var bodyBuff bytes.Buffer
		mulWriter := multipart.NewWriter(&bodyBuff)
		bodyTpl := FromGoRet[map[string]interface{}](xmap.GetDeep[map[string]interface{}](u.Definition, "http.request.body")).ValueOrExit()
		for fieldName, fieldMeta_ := range bodyTpl {
			GVerbose.Trace("processing field: " + fieldName)
			fieldMeta := fieldMeta_.(map[string]interface{})
			fieldType := fieldMeta["type"]
			if fieldType == "string" {
				fieldValue := u.replaceRequest(fieldMeta["value"].(string), tplData)
				mulWriter.WriteField(fieldName, fieldValue)
				GVerbose.Trace("field value: " + fieldValue)
			} else if fieldType == "file" {
				fileName := filepath.Base(task.LocalPath)
				part := FromGoRet[io.Writer](mulWriter.CreateFormFile(fieldName, fileName)).ValueOrExit()
				n, err := part.Write(FromGoRet[[]byte](ioutil.ReadFile(task.LocalPath)).ValueOrExit())
				abortErr(err)
				GVerbose.Trace("field value: "+"[file (len=%d, name=%s)]", n, fileName)
			}
		}
		headerCache.Set("Content-Type", mulWriter.FormDataContentType())
		mulWriter.Close()
		body = ioutil.NopCloser(bytes.NewReader(bodyBuff.Bytes()))
	}

	// == Create Request ==
	req := FromGoRet[*http.Request](http.NewRequest(method, u.replaceRequest(url, tplData), body)).ValueOrExit()
	req.Header = headerCache
	GVerbose.Trace("do headers:")
	GVerbose.TraceStruct(map[string][]string(req.Header))

	// == Do Request ==
	resp := FromGoRet[*http.Response](http.DefaultClient.Do(req)).ValueOrExit()

	// == Construct rawUrl from Response ==
	urlFrom := FromGoRet[string](xmap.GetDeep[string](u.Definition, "upload.rawUrl.from")).ValueOrExit()
	if urlFrom == "json_response" {
		// read response body json
		bodyBytes := FromGoRet[[]byte](ioutil.ReadAll(resp.Body)).ValueOrExit()
		GVerbose.Info("response body:" + string(bodyBytes))
		var respJson map[string]interface{}
		err := json.Unmarshal(bodyBytes, &respJson)
		if err != nil {
			return "", errors.New("json response is not valid")
		}
		if !(resp.StatusCode <= 200 && resp.StatusCode < 300) {
			return "", fmt.Errorf("response status code %d is not expected. resp: %s", resp.StatusCode, string(bodyBytes))
		}
		rawUrlPath := FromGoRet[string](xmap.GetDeep[string](u.Definition, "upload.rawUrl.path")).ValueOrExit()
		rawUrl, err = xmap.GetDeep[string](respJson, rawUrlPath)
		if len(rawUrl) == 0 {
			return "", fmt.Errorf("unable to get url. resp: %s", string(bodyBytes))
		}
		GVerbose.Trace("got rawUrl from resp: " + rawUrl)
	} else {
		return "", errors.New("unsupported rawUrl from type")
	}
	return
}

func (u SimpleHttpUploader) Upload(t *Task) (ret Result[*Task]) {
	now := time.Now()
	base := xpath.Basename(t.LocalPath)

	if len(t.TargetDir) > 0 {
		t.TargetPath = t.TargetDir + "/" + base
	} else {
		t.TargetPath = Rename(base, now)
	}
	GVerbose.Trace("uploading #TASK_%d %s\n", t.TaskId, t.LocalPath)
	// var err error
	rawUrl, err := u.UploadFile(t)
	var url string
	if err == nil {
		url := ReplaceUrl(rawUrl)
		GVerbose.Trace("sucessfully uploaded #TASK_%d %s => %s\n", t.TaskId, t.LocalPath, url)
		t.Status = TASK_FINISHED
	} else {
		GVerbose.Trace("failed to upload #TASK_%d %s : %s\n", t.TaskId, t.LocalPath, err.Error())
		t.Status = TASK_FAILED
	}
	t.RawUrl = rawUrl
	t.Url = url
	t.FinishTime = now
	ret = Result[*Task]{
		err:   err,
		value: t,
	}
	return
}
