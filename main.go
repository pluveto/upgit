package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"syscall"
	"time"

	"github.com/alexflint/go-arg"
	"github.com/pelletier/go-toml/v2"
	"github.com/pluveto/upgit/lib/model"
	"github.com/pluveto/upgit/lib/qcloudcos"
	"github.com/pluveto/upgit/lib/result"
	"github.com/pluveto/upgit/lib/uploaders"
	"github.com/pluveto/upgit/lib/upyun"
	"github.com/pluveto/upgit/lib/xapp"
	"github.com/pluveto/upgit/lib/xclipboard"
	"github.com/pluveto/upgit/lib/xext"
	"github.com/pluveto/upgit/lib/xgithub"
	"github.com/pluveto/upgit/lib/xhttp"
	"github.com/pluveto/upgit/lib/ximage"
	"github.com/pluveto/upgit/lib/xio"
	"github.com/pluveto/upgit/lib/xlog"
	"github.com/pluveto/upgit/lib/xmap"
	"github.com/pluveto/upgit/lib/xpath"
	"github.com/pluveto/upgit/lib/xstrings"
	"github.com/pluveto/upgit/lib/xzip"
	"golang.design/x/clipboard"
	"gopkg.in/validator.v2"
)

func main() {
	result.AbortErr = xlog.AbortErr
	if len(os.Args) >= 2 && os.Args[1] == "ext" {
		extSubcommand()
		return
	}
	mainCommand()
}

func mainCommand() {
	// parse cli args
	loadCliOpts()

	// load config
	loadEnvConfig(&xapp.AppCfg)
	loadConfig(&xapp.AppCfg)

	xlog.GVerbose.TraceStruct(xapp.AppCfg)

	// handle clipboard if need
	handleClipboard()

	// validating args
	validArgs()
	var tmpFiles []string
	defer func() {
		for _, path := range tmpFiles {
			os.Remove(path)
		}
	}()
	handleScaleFix(tmpFiles)
	// executing uploading
	dispatchUploader()

	if xapp.AppOpt.Wait {
		fmt.Scanln()
	}
}

func handleScaleFix(tmpFiles []string) {

	scaleFix := GetScaleFix()
	if scaleFix != 100 {
		// read file to buf
		for i, path := range xapp.AppOpt.LocalPaths {
			newTmpPath := path + ".upgit_tmp.png"

			buf, err := xio.ReadFile(path)
			if err != nil {
				xlog.AbortErr(err)
				return
			}

			if !(xapp.AppCfg.ScaleFix > 100 && xapp.AppCfg.ScaleFix <= 200) {
				xlog.AbortErr(fmt.Errorf("failed: scale fix must be in range 100-200"))
			}
			factor := 100.0 / float32(scaleFix)
			width, height, err := ximage.GetSize(buf)
			if err != nil {
				xlog.AbortErr(fmt.Errorf("failed: failed to get image size: %s", err.Error()))
			}
			newWidth := uint(float32(width) * factor)
			newHeight := uint(float32(height) * factor)
			buf, err = ximage.Scale(buf, newWidth, newHeight)
			if nil == buf || err != nil {
				xlog.AbortErr(fmt.Errorf("failed: scale image failed to size %d x %d", width, height))
				xlog.AbortErr(err)
			}
			xlog.GVerbose.Info("scale fix: %d%%, from %d x %d to %d x %d", xapp.AppCfg.ScaleFix, width, height, newWidth, newHeight)

			// write to tmp file
			err = xio.WriteFile(newTmpPath, buf)
			if err != nil {
				xlog.AbortErr(err)
				return
			}
			xapp.AppOpt.LocalPaths[i] = newTmpPath
			tmpFiles = append(tmpFiles, newTmpPath)
		}
	}
}

// loadCliOpts load cli options into xapp.AppOpt
func loadCliOpts() {
	arg.MustParse(&xapp.AppOpt)
	xapp.AppOpt.TargetDir = strings.Trim(xapp.AppOpt.TargetDir, "/")
	xapp.AppOpt.ApplicationPath = strings.Trim(xapp.AppOpt.ApplicationPath, "/")
	if len(xapp.AppOpt.ApplicationPath) > 0 {
		xpath.ApplicationPath = xapp.AppOpt.ApplicationPath
	}
	if xapp.AppOpt.SizeLimit != nil && *xapp.AppOpt.SizeLimit >= 0 {
		xapp.MaxUploadSize = *xapp.AppOpt.SizeLimit
	}
	if false == xapp.AppOpt.NoLog {
		xlog.GVerbose.LogEnabled = true
		xlog.GVerbose.LogFile = xpath.MustGetApplicationPath("upgit.log")
		xlog.GVerbose.LogFileMaxSize = 2 * 1024 * 1024 // 2MiB
		xlog.GVerbose.Info("Started")
		xlog.GVerbose.TruncatLog()
	}
	xlog.GVerbose.VerboseEnabled = xapp.AppOpt.Verbose
	xlog.GVerbose.TraceStruct(xapp.AppOpt)
}

func onUploaded(r result.Result[*model.Task]) {
	if !r.Ok() && xapp.AppOpt.OutputType == xapp.O_Stdout {
		fmt.Println("Failed: " + r.Err.Error())
		return
	}
	if xapp.AppOpt.Clean && !r.Value.Ignored {
		err := os.Remove(r.Value.LocalPath)
		if err != nil {
			xlog.GVerbose.Info("Failed to remove %s: %s", r.Value.LocalPath, err.Error())
		} else {
			xlog.GVerbose.Info("Removed %s", r.Value.LocalPath)
		}

	}
	outputLink(*r.Value)
	recordHistory(*r.Value)
}

func mustMarshall(s interface{}) string {
	b, err := toml.Marshal(s)
	if err != nil {
		return ""
	}
	return string(b)
}

func recordHistory(r model.Task) {
	xio.AppendToFile(xpath.MustGetApplicationPath("history.log"), []byte(
		`{"time":"`+time.Now().Local().String()+`","rawUrl":"`+r.RawUrl+`","url":"`+r.Url+`"}`+"\n"),
	)

	xlog.GVerbose.Info(mustMarshall(r))
}

func outputLink(r model.Task) {
	outContent, err := outputFormat(r)
	xlog.AbortErr(err)
	switch xapp.AppOpt.OutputType {
	case xapp.O_Stdout:
		fmt.Println(outContent)
	case xapp.O_Clipboard:
		clipboard.Write(clipboard.FmtText, []byte(outContent))
	default:
		xlog.AbortErr(errors.New("unknown output type: " + string(xapp.AppOpt.OutputType)))
	}
}

func outputFormat(r model.Task) (content string, err error) {
	var outUrl string
	if xapp.AppOpt.Raw || r.Url == "" {
		outUrl = r.RawUrl
	} else {
		outUrl = r.Url
	}
	fmt := xapp.AppOpt.OutputFormat
	if fmt == "" {
		return outUrl, nil
	}
	val, ok := xapp.AppCfg.OutputFormats[fmt]
	if !ok {
		return "", errors.New("unknown output format: " + fmt)
	}
	content = strings.NewReplacer(
		"{url}", outUrl,
		"{urlfname}", filepath.Base(outUrl),
		"{fname}", filepath.Base(r.LocalPath),
	).Replace(xstrings.RemoveFmtUnderscore(val))

	return
}

func validArgs() {
	if errs := validator.Validate(xapp.AppCfg); errs != nil {
		xlog.AbortErr(fmt.Errorf("incorrect config: " + errs.Error()))
	}

	for _, path := range xapp.AppOpt.LocalPaths {
		if strings.HasPrefix(path, "http") {
			continue
		}
		fs, err := os.Stat(path)
		if errors.Is(err, os.ErrNotExist) {
			xlog.AbortErr(fmt.Errorf("invalid file to upload %s: no such file", path))
		}
		if err != nil {
			xlog.AbortErr(fmt.Errorf("invalid file to upload %s: %s", path, err.Error()))
		}
		if fs.Size() == 0 {
			xlog.AbortErr(fmt.Errorf("invalid file to upload %s: file size is zero", path))
		}
		if xapp.MaxUploadSize != 0 && fs.Size() > xapp.MaxUploadSize {
			xlog.AbortErr(fmt.Errorf("invalid file to upload %s: file size is larger than %d bytes", path, xapp.MaxUploadSize))
		}
	}
}

// loadConfig loads config from config file to xapp.AppCfg
func loadConfig(cfg *xapp.Config) {

	homeDir, err := os.UserHomeDir()
	if err != nil {
		homeDir = ""
	}

	appDir := xpath.MustGetApplicationPath("")

	var configFiles = map[string]bool{
		filepath.Join(homeDir, ".upgit.config.toml"):                false,
		filepath.Join(homeDir, filepath.Join(".config", "upgitrc")): false,
		filepath.Join(appDir, "config.toml"):                        false,
		filepath.Join(appDir, "upgit.toml"):                         false,
	}

	if xapp.AppOpt.ConfigFile != "" {
		configFiles[xapp.AppOpt.ConfigFile] = true
	}

	for configFile, required := range configFiles {
		if _, err := os.Stat(configFile); err != nil {
			if required {
				xlog.AbortErr(fmt.Errorf("config file %s not found", configFile))
			}
			continue
		}
		optRawBytes, err := ioutil.ReadFile(configFile)
		if err == nil {
			err = toml.Unmarshal(optRawBytes, &cfg)
		}
		if err != nil {
			xlog.AbortErr(fmt.Errorf("invalid config: " + err.Error()))
		}
		xapp.ConfigFilePath = configFile
		break
	}

	if xapp.ConfigFilePath == "" {
		xlog.AbortErr(fmt.Errorf("no config file found"))
	}

	// fill config
	xapp.AppCfg.Rename = strings.Trim(xapp.AppCfg.Rename, "/")
	xapp.AppCfg.Rename = xstrings.RemoveFmtUnderscore(xapp.AppCfg.Rename)

	// -- integrated formats
	if nil == xapp.AppCfg.OutputFormats {
		xapp.AppCfg.OutputFormats = make(map[string]string)
	}
	xapp.AppCfg.OutputFormats["markdown"] = `![{url_fname}]({url})`
	xapp.AppCfg.OutputFormats["url"] = `{url}`

}

// UploadAll will upload all given file to targetDir.
// If targetDir is not set, it will upload using rename rules.
func UploadAll(uploader model.Uploader, localPaths []string, targetDir string) {
	for taskId, localPath := range localPaths {

		var ret result.Result[*model.Task]
		task := model.Task{
			Status:     model.TASK_CREATED,
			TaskId:     taskId,
			LocalPath:  localPath,
			TargetDir:  targetDir,
			RawUrl:     "",
			Url:        "",
			CreateTime: time.Now(),
		}
		var err error
		// ignore non-local path
		if strings.HasPrefix(localPath, "http") {
			task.Ignored = true
			task.Status = model.TASK_FINISHED
		} else {
			err = uploader.Upload(&task)
		}
		if err != nil {
			task.Status = model.TASK_FAILED
			ret = result.Result[*model.Task]{
				Err: err,
			}
		} else {
			ret = result.Result[*model.Task]{
				Value: &task,
			}
		}

		if err == nil {
			xlog.GVerbose.TraceStruct(ret.Value)
		}
		callback := uploader.GetCallback()
		if nil != callback {
			callback(ret)
		}
	}
}

func dispatchUploader() {
	uploaderId := xstrings.ValueOrDefault(xapp.AppOpt.Uploader, xapp.AppCfg.DefaultUploader)
	xlog.GVerbose.Info("uploader: " + uploaderId)
	if uploaderId == "github" {
		gCfg, err := xapp.LoadUploaderConfig[uploaders.GithubUploaderConfig](uploaderId)
		xlog.AbortErr(err)
		err = validator.Validate(&gCfg)
		xlog.AbortErr(err)
		if len(gCfg.Branch) == 0 {
			gCfg.Branch = xapp.DefaultBranch
		}

		uploader := uploaders.GithubUploader{Config: gCfg, OnTaskStatusChanged: onUploaded}
		UploadAll(uploader, xapp.AppOpt.LocalPaths, xapp.AppOpt.TargetDir)
		return
	}
	if uploaderId == "qcloudcos" {
		qCfg, err := xapp.LoadUploaderConfig[qcloudcos.COSConfig](uploaderId)
		xlog.AbortErr(err)
		err = validator.Validate(&qCfg)
		xlog.AbortErr(err)
		xlog.GVerbose.Trace("qcloudcos config: ")
		xlog.GVerbose.TraceStruct(&qCfg)
		uploader := qcloudcos.COSUploader{Config: qCfg, OnTaskStatusChanged: onUploaded}
		UploadAll(uploader, xapp.AppOpt.LocalPaths, xapp.AppOpt.TargetDir)
		return
	}
	if uploaderId == "upyun" {
		ucfg, err := xapp.LoadUploaderConfig[upyun.UpyunConfig](uploaderId)
		xlog.AbortErr(err)
		err = validator.Validate(&ucfg)
		xlog.AbortErr(err)
		xlog.GVerbose.Trace("qcloudcos config: ")
		xlog.GVerbose.TraceStruct(&ucfg)
		uploader := upyun.UpyunUploader{Config: ucfg, OnTaskStatusChanged: onUploaded}
		UploadAll(uploader, xapp.AppOpt.LocalPaths, xapp.AppOpt.TargetDir)
		return
	}
	// try http simple uploader
	// list file in ./extensions
	extDir := xpath.MustGetApplicationPath("extensions")
	info, err := ioutil.ReadDir(extDir)
	xlog.AbortErr(err)
	var uploader *uploaders.SimpleHttpUploader
	for _, f := range info {
		fname := f.Name()
		xlog.GVerbose.Trace("found file %s", fname)
		if !strings.HasSuffix(fname, ".json") && !strings.HasSuffix(fname, ".jsonc") {
			xlog.GVerbose.Trace("ignored file %s", fname)
			continue
		}
		// load file to json
		uploaderDef, err := xext.GetExtDefinitionInterface(extDir, fname)
		xlog.AbortErr(err)
		if result.From[string](xmap.GetDeep[string](uploaderDef, `meta.id`)).ValueOrExit() != uploaderId {
			continue
		}
		if result.From[string](xmap.GetDeep[string](uploaderDef, "meta.type")).ValueOrExit() != "simple-http-uploader" {
			continue
		}
		uploader = &uploaders.SimpleHttpUploader{OnTaskStatusChanged: onUploaded, Definition: uploaderDef}
		extConfig, err := xapp.LoadUploaderConfig[map[string]interface{}](uploaderId)
		if err == nil {
			uploader.Config = extConfig
			xlog.GVerbose.Trace("uploader config:")
			xlog.GVerbose.TraceStruct(uploader.Config)
		} else {
			xlog.GVerbose.Trace("no uploader config found")
		}
		break
	}
	if nil == uploader {
		xlog.AbortErr(errors.New("unknown uploader: " + uploaderId))
	}
	UploadAll(uploader, xapp.AppOpt.LocalPaths, xapp.AppOpt.TargetDir)

}

func handleClipboard() {
	if len(xapp.AppOpt.LocalPaths) == 1 {
		label := strings.ToLower(xapp.AppOpt.LocalPaths[0])
		if label == xapp.ClipboardPlaceholder {
			err := clipboard.Init()
			if err != nil {
				xlog.AbortErr(fmt.Errorf("failed to init clipboard: " + err.Error()))
			}

			tmpFileName := fmt.Sprint(os.TempDir(), "/upgit_tmp_", time.Now().UnixMicro(), ".png")
			buf := clipboard.Read(clipboard.FmtImage)
			if nil == buf {
				// try second chance for Windows user. To adapt bitmap format (compatible with Snipaste)
				if runtime.GOOS == "windows" {
					buf, err = xclipboard.ReadClipboardImage()
				}
				if err != nil {
					xlog.GVerbose.Error("failed to read clipboard image: " + err.Error())
				}
			}
			if nil == buf {
				xlog.AbortErr(fmt.Errorf("failed: no image in clipboard or unsupported format"))
			}
			os.WriteFile(tmpFileName, buf, os.FileMode(fs.ModePerm))
			xapp.AppOpt.LocalPaths[0] = tmpFileName
			xapp.AppOpt.Clean = true
		}
		if strings.HasPrefix(label, xapp.ClipboardFilePlaceholder) {
			// Must be Windows
			if runtime.GOOS != "windows" {
				xlog.AbortErr(fmt.Errorf("failed: clipboard file only supported on Windows"))
			}
			// Download latest https://github.com/pluveto/APIProxy-Win32/releases
			// and put it in same directory with upgit.exe
			download := func() {
				downloadUrl, err := xgithub.GetLatestReleaseDownloadUrl("pluveto/APIProxy-Win32")
				xlog.AbortErr(err)
				xlog.GVerbose.Trace("download url: %s", downloadUrl)
				saveName := xpath.MustGetApplicationPath("/apiproxy-win32.zip")
				xlog.AbortErr(xhttp.DownloadFile(downloadUrl, saveName))
				// Unzip
				xlog.AbortErr(xzip.Unzip(saveName, xpath.MustGetApplicationPath("/")))
				// Clean downloaded zip
				xlog.AbortErr(os.Remove(saveName))
			}
			// Run
			executable := xpath.MustGetApplicationPath("APIProxy.exe")
			if _, err := os.Stat(executable); os.IsNotExist(err) {
				println("APIProxy not found, downloading...")
				download()
			}
			execArgs := []string{"clipboard", "GetFilePaths"}
			cmd := exec.Command(executable)
			cmd.Args = append(cmd.Args, execArgs...)
			// Wait and fetch cmdOutput
			cmdOutput, err := cmd.Output()
			if err != nil {
				xlog.AbortErr(fmt.Errorf("failed to run APIProxy: %s, stderr: %s", err.Error(), cmdOutput))
			}
			parseOutput := func(output string) []string {
				/*
				   Type: ApplicationError
				   Msg: No handler name specified
				   HMsg:
				   Data: null
				*/
				lines := strings.Split(output, "\n")
				for i, line := range lines {
					lines[i] = strings.TrimSpace(line)
					if len(lines[i]) == 0 {
						lines = append(lines[:i], lines[i+1:]...)
					}
				}
				var result []string
				if len(lines) != 4 {
					xlog.AbortErr(errors.New("unable to parse APIProxy output, unexpected line count. output: " + output))
					return result
				}
				if !strings.HasPrefix(lines[0], "Type: Success") {
					xlog.AbortErr(errors.New("got error from APIProxy output: " + output))
					return result
				}
				// Parse data
				jsonStr := lines[3][len("Data: "):]
				var paths []string
				xlog.AbortErr(json.Unmarshal([]byte(jsonStr), &paths))
				return paths
			}
			xlog.GVerbose.Trace("stdout: %s", cmdOutput)
			paths := parseOutput(string(cmdOutput))
			if len(paths) == 0 {
				xlog.AbortErr(errors.New("no file in clipboard"))
			}
			xapp.AppOpt.LocalPaths = paths
		}
	}
}

func GetScaleFix() int {
	scaleFix := 100
	if xapp.AppCfg.ScaleFix != 0 && xapp.AppCfg.ScaleFix != 100 {
		scaleFix = xapp.AppCfg.ScaleFix
	}
	if xapp.AppOpt.ScaleFix != nil && *xapp.AppOpt.ScaleFix != 100 {
		scaleFix = *xapp.AppOpt.ScaleFix
	}
	return scaleFix
}

func loadEnvConfig(cfg *xapp.Config) {
	if nil == cfg {
		xlog.AbortErr(fmt.Errorf("unable to load env config: nil config"))
	}

	if rename, found := syscall.Getenv("UPGIT_RENAME"); found {
		cfg.Rename = rename
	}
}

func loadGithubUploaderEnvConfig(gCfg *uploaders.GithubUploaderConfig) {
	// TODO: Auto generate env key name and adapt for all uploaders
	if pat, found := syscall.Getenv("GITHUB_TOKEN"); found {
		gCfg.PAT = pat
	}
	if pat, found := syscall.Getenv("UPGIT_TOKEN"); found {
		gCfg.PAT = pat
	}
	if username, found := syscall.Getenv("UPGIT_USERNAME"); found {
		gCfg.Username = username
	}
	if repo, found := syscall.Getenv("UPGIT_REPO"); found {
		gCfg.Repo = repo
	}
	if branch, found := syscall.Getenv("UPGIT_BRANCH"); found {
		gCfg.Branch = branch
	}
}
