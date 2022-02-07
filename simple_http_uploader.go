package main

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"io/ioutil"
	"mime/multipart"
	"net/http"
	"net/url"
	"path/filepath"
	"reflect"
	"strings"
	"time"

	"github.com/pluveto/upgit/lib/model"
	"github.com/pluveto/upgit/lib/result"
	"github.com/pluveto/upgit/lib/xapp"
	"github.com/pluveto/upgit/lib/xlog"
	"github.com/pluveto/upgit/lib/xmap"
	"github.com/pluveto/upgit/lib/xpath"
	"github.com/pluveto/upgit/lib/xstrings"
)

type SimpleHttpUploader struct {
	Config              map[string]interface{}
	Definition          map[string]interface{}
	OnTaskStatusChanged func(result.Result[*model.Task])
}

func (u SimpleHttpUploader) SetCallback(f func(result.Result[*model.Task])) {
	u.OnTaskStatusChanged = f
}

func (u SimpleHttpUploader) GetCallback() func(result.Result[*model.Task]) {
	return u.OnTaskStatusChanged
}

// func (u SimpleHttpUploader) UploadAll(localPaths []string, targetDir string) {
// 	for taskId, localPath := range localPaths {

// 		var ret Result[*model.Task]
// 		task := model.Task{
// 			Status:     TASK_CREATED,
// 			TaskId:     taskId,
// 			LocalPath:  localPath,
// 			TargetDir:  targetDir,
// 			RawUrl:     localPath,
// 			Url:        localPath,
// 			FinishTime: time.Now(),
// 		}
// 		// ignore non-local path
// 		if strings.HasPrefix(localPath, "http") {
// 			task.Ignored = true
// 			task.Status = TASK_FINISHED
// 			ret = Result[*model.Task]{
// 				value: &task,
// 			}
// 		} else {
// 			ret = u.Upload(&task)
// 		}

// 		if ret.Err == nil {
// 			xlog.GVerbose.TraceStruct(ret.Value)
// 		}
// 		if nil != u.OnTaskStatusChanged {
// 			u.OnTaskStatusChanged(ret)
// 		}
// 	}
// }

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

var ConfigDelimiters = []string{"$(", ")"}

func (u SimpleHttpUploader) replaceStringPlaceholder(s string, task *model.Task) string {
	dict := make(map[string]interface{}, 1)
	dict["_"] = s
	u.replaceDictPlaceholder(dict, task)
	xlog.GVerbose.Trace("replaceStringPlaceholder: %s => %s", s, dict["_"])
	return dict["_"].(string)
}

func (u SimpleHttpUploader) replaceDictPlaceholder(data map[string]interface{}, task *model.Task) {
	for k, v_ := range data {
		if reflect.TypeOf(v_).Kind() != reflect.String {
			// xlog.GVerbose.Trace("skip non-string value: " + k)
			continue
		}
		v := v_.(string)

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
				ret = GetValueByConfigTag(&xapp.AppCfg, subKey).(string)
				return &ret

			} else if parentKey == "option" {
				ret = GetValueByConfigTag(&xapp.AppOpt, subKey).(string)
				return &ret

			} else if parentKey == "task" {
				ret = GetValueByConfigTag(task, subKey).(string)
				return &ret
			}
			return nil
		}

		ret := xstrings.VariableReplaceFunc(v, ConfigDelimiters[0], ConfigDelimiters[1], replacer)
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

func (u SimpleHttpUploader) UploadFile(task *model.Task) (rawUrl string, err error) {
	// TODO: Do not use tplData, use stantard $(task.remote_path)
	tplData := map[string]string{
		"remote_path": task.TargetPath,
	}
	// == prepare method and url ==
	method := result.FromGoRet[string](xmap.GetDeep[string](u.Definition, "http.request.method")).ValueOrExit()
	urlRaw := result.FromGoRet[string](xmap.GetDeep[string](u.Definition, "http.request.url")).ValueOrExit()
	params := result.FromGoRet[map[string]interface{}](xmap.GetDeep[map[string]interface{}](u.Definition, "http.request.params")).ValueOrDefault(map[string]interface{}{})
	u.replaceDictPlaceholder(params, task)
	url := result.FromGoRet[*url.URL](url.Parse(u.replaceStringPlaceholder(urlRaw, task))).ValueOrExit()
	query := url.Query()
	for paramName, paramValue := range params {
		query.Add(paramName, paramValue.(string))
	}
	url.RawQuery = query.Encode()
	xlog.GVerbose.Info("Method: %s, URL: %s", method, url.String())

	//  == Prepare header ==
	headers := result.FromGoRet[map[string]interface{}](xmap.GetDeep[map[string]interface{}](u.Definition, "http.request.headers")).ValueOrExit()
	u.replaceDictPlaceholder(headers, task)

	xlog.GVerbose.Trace("unformatted headers:")
	xlog.GVerbose.TraceStruct(headers)
	headerCache := make(http.Header)
	for k, v := range headers {
		headerCache.Set(k, u.replaceRequest(v.(string), tplData))
	}
	if headerCache.Get("Content-Type") == "" {
		headerCache.Set("Content-Type", "application/octet-stream")
	}
	xlog.GVerbose.Trace("formatted headers:")
	xlog.GVerbose.TraceStruct(map[string][]string(headerCache))
	// upload file according to content-type
	contentType := headerCache.Get("Content-Type")

	// == Prepare body ==
	var body io.ReadCloser
	if contentType == "application/octet-stream" {
		body = ioutil.NopCloser(bytes.NewReader(result.FromGoRet[[]byte](ioutil.ReadFile(task.LocalPath)).ValueOrExit()))
	} else if contentType == "multipart/form-data" {
		var bodyBuff bytes.Buffer
		mulWriter := multipart.NewWriter(&bodyBuff)
		bodyTpl := result.FromGoRet[map[string]interface{}](xmap.GetDeep[map[string]interface{}](u.Definition, "http.request.body")).ValueOrExit()
		for fieldName, fieldMeta_ := range bodyTpl {
			xlog.GVerbose.Trace("processing field: " + fieldName)
			fieldMeta := fieldMeta_.(map[string]interface{})
			fieldType := fieldMeta["type"]

			if fieldType == "string" {
				fieldValue := u.replaceRequest(fieldMeta["value"].(string), tplData)
				fieldValue = u.replaceStringPlaceholder(fieldValue, task)
				mulWriter.WriteField(fieldName, fieldValue)
				xlog.GVerbose.Trace("field(string) value: " + fieldValue)

			} else if fieldType == "file" {
				fileName := filepath.Base(task.LocalPath)
				part := result.FromGoRet[io.Writer](mulWriter.CreateFormFile(fieldName, fileName)).ValueOrExit()
				n, err := part.Write(result.FromGoRet[[]byte](ioutil.ReadFile(task.LocalPath)).ValueOrExit())
				xlog.AbortErr(err)
				xlog.GVerbose.Trace("field(file) value: "+"[file (len=%d, name=%s)]", n, fileName)

			} else if fieldType == "file_base64" {
				dat, err := ioutil.ReadFile(task.LocalPath)
				if err != nil {
					return "", err
				}
				encoded := base64.StdEncoding.EncodeToString(dat)
				mulWriter.WriteField(fieldName, encoded)
			}
		}
		headerCache.Set("Content-Type", mulWriter.FormDataContentType())
		mulWriter.Close()
		body = ioutil.NopCloser(bytes.NewReader(bodyBuff.Bytes()))
	}

	// == Create Request ==
	req := result.FromGoRet[*http.Request](http.NewRequest(method, u.replaceRequest(url.String(), tplData), body)).ValueOrExit()
	req.Header = headerCache
	xlog.GVerbose.Trace("do headers:")
	xlog.GVerbose.TraceStruct(map[string][]string(req.Header))

	// == Do Request ==
	resp := result.FromGoRet[*http.Response](http.DefaultClient.Do(req)).ValueOrExit()
	bodyBytes := result.FromGoRet[[]byte](ioutil.ReadAll(resp.Body)).ValueOrExit()
	xlog.GVerbose.Info("response body:" + string(bodyBytes))
	// == Construct rawUrl from Response ==
	urlFrom := result.FromGoRet[string](xmap.GetDeep[string](u.Definition, "upload.rawUrl.from")).ValueOrExit()
	if urlFrom == "json_response" {
		// read response body json

		var respJson map[string]interface{}
		err := json.Unmarshal(bodyBytes, &respJson)
		if err != nil {
			return "", errors.New("json response is not valid")
		}
		if !(resp.StatusCode <= 200 && resp.StatusCode < 300) {
			return "", fmt.Errorf("response status code %d is not expected. resp: %s", resp.StatusCode, string(bodyBytes))
		}
		rawUrlPath := result.FromGoRet[string](xmap.GetDeep[string](u.Definition, "upload.rawUrl.path")).ValueOrExit()
		rawUrl, err = xmap.GetDeep[string](respJson, rawUrlPath)
		if len(rawUrl) == 0 {
			return "", fmt.Errorf("unable to get url. resp: %s", string(bodyBytes))
		}
		xlog.GVerbose.Trace("got rawUrl from resp: " + rawUrl)
	} else {
		return "", errors.New("unsupported rawUrl from type")
	}
	return
}

func (u SimpleHttpUploader) Upload(t *model.Task) (err error) {
	now := time.Now()
	base := xpath.Basename(t.LocalPath)

	if len(t.TargetDir) > 0 {
		t.TargetPath = t.TargetDir + "/" + base
	} else {
		t.TargetPath = xapp.Rename(base, now)
	}
	xlog.GVerbose.Trace("uploading #TASK_%d %s\n", t.TaskId, t.LocalPath)
	// var err error
	rawUrl, err := u.UploadFile(t)
	var url string
	if err == nil {
		url := xapp.ReplaceUrl(rawUrl)
		xlog.GVerbose.Trace("sucessfully uploaded #TASK_%d %s => %s\n", t.TaskId, t.LocalPath, url)
		t.Status = model.TASK_FINISHED
	} else {
		xlog.GVerbose.Trace("failed to upload #TASK_%d %s : %s\n", t.TaskId, t.LocalPath, err.Error())
		t.Status = model.TASK_FAILED
	}
	t.RawUrl = rawUrl
	t.Url = url
	t.FinishTime = now
	return
}
