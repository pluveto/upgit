package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"io/ioutil"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"syscall"
	"time"

	"github.com/alexflint/go-arg"
	"github.com/mitchellh/mapstructure"
	"github.com/pelletier/go-toml/v2"
	"github.com/pluveto/upgit/lib/xclipboard"
	"github.com/pluveto/upgit/lib/xmap"
	"golang.design/x/clipboard"
	"gopkg.in/validator.v2"
)

type OutputType string

const (
	O_Stdout    OutputType = "stdout"
	O_Clipboard            = "clipboard"
)

const kDefaultBranch = "master"
const kClipboardPlaceholder = ":clipboard"
const kRepoURL = "https://github.com/pluveto/upgit"

var maxUploadSize = int64(5 * 1024 * 1024)

type CLIOptions struct {
	LocalPaths   []string   `arg:"positional, required" placeholder:"FILE" help:"local file path to upload. :clipboard for uploading clipboard image"`
	TargetDir    string     `arg:"-t,--target-dir"    help:"upload file with original name to given directory. if not set, will use renaming rules"`
	Verbose      bool       `arg:"-V,--verbose"       help:"when set, output more details to help developers"`
	SizeLimit    *int64     `arg:"-s,--size-limit"    help:"in bytes. overwrite default size limit (5MiB). 0 means no limit"`
	Wait         bool       `arg:"-w,--wait"          help:"when set, not exit after upload, util user press any key"`
	Clean        bool       `arg:"-c,--clean"         help:"when set, remove local file after upload"`
	Raw          bool       `arg:"-r,--raw"           help:"when set, output non-replaced raw url"`
	NoLog        bool       `arg:"-n,--no-log"        help:"when set, disable logging"`
	Uploader     string     `arg:"-u,--uploader"      help:"uploader to use. if not set, will follow config"`
	OutputType   OutputType `arg:"-o,--output-type"   help:"output type, supports stdout, clipboard" default:"stdout"`
	OutputFormat string     `arg:"-f,--output-format" help:"output format, supports url, markdown and your customs" default:"url"`
}

func (CLIOptions) Description() string {
	return "\n" +
		"Upload anything to github repo or other remote storages and then get its link.\n" +
		"For more information: " + kRepoURL + "\n"
}

type Config struct {
	DefaultUploader string            `toml:"default_uploader,omitempty"`
	Rename          string            `toml:"rename,omitempty"`
	Replacements    map[string]string `toml:"replacements,omitempty"`
	OutputFormats   map[string]string `toml:"output_formats,omitempty"`
}

type GithubUploaderConfig struct {
	PAT      string `toml:"pat" validate:"nonzero"`
	Username string `toml:"username" validate:"nonzero"`
	Repo     string `toml:"repo" validate:"nonzero"`
	Branch   string `toml:"branch,omitempty"`
}

var opt CLIOptions
var cfg Config

var configFilePath string

func main() {

	// parse cli args
	arg.MustParse(&opt)
	opt.TargetDir = strings.Trim(opt.TargetDir, "/")
	if opt.SizeLimit != nil && *opt.SizeLimit >= 0 {
		maxUploadSize = *opt.SizeLimit
	}
	if false == opt.NoLog {
		GVerbose.LogEnabled = true
		GVerbose.LogFile = MustGetApplicationPath("upgit.log")
		GVerbose.Info("Started")
	}
	GVerbose.VerboseEnabled = opt.Verbose
	GVerbose.TraceStruct(opt)

	// load config
	loadEnvConfig(&cfg)
	loadTomlConfig(&cfg)

	// fill config
	cfg.Rename = strings.Trim(cfg.Rename, "/")
	cfg.Rename = RemoveFmtUnderscore(cfg.Rename)

	// -- integrated formats
	if nil == cfg.OutputFormats {
		cfg.OutputFormats = make(map[string]string)
	}
	cfg.OutputFormats["markdown"] = `![{url_fname}]({url})`
	cfg.OutputFormats["url"] = `{url}`
	GVerbose.TraceStruct(cfg)

	// handle clipboard if need
	loadClipboard()

	// validating args
	validArgs()

	// executing uploading
	upload()

	if opt.Wait {
		fmt.Scanln()
	}

	return
}

func onUploaded(r Result[*Task]) {
	if !r.Ok() && opt.OutputType == O_Stdout {
		fmt.Println("Failed: " + r.err.Error())
		GVerbose.Info("Failed to upload %s: %s", r.value.LocalPath, r.err.Error())
		return
	}
	if opt.Clean && !r.value.Ignored {
		err := os.Remove(r.value.LocalPath)
		if err != nil {
			GVerbose.Info("Failed to remove %s: %s", r.value.LocalPath, err.Error())
		} else {
			GVerbose.Info("Removed %s", r.value.LocalPath)
		}

	}
	outputLink(*r.value)
	recordHistory(*r.value)
}

func recordHistory(r Task) {
	os.WriteFile(MustGetApplicationPath("history.log"), []byte(
		`{"time":"`+time.Now().Local().String()+`","rawUrl":"`+r.RawUrl+`","url":"`+r.Url+`"}`),
		os.ModeAppend,
	)
	GVerbose.Info(MustMarshall(r))
}

func outputLink(r Task) {
	outContent, err := outputFormat(r)
	abortErr(err)
	switch opt.OutputType {
	case O_Stdout:
		fmt.Println(outContent)
	case O_Clipboard:
		clipboard.Write(clipboard.FmtText, []byte(outContent))
	default:
		abortErr(errors.New("unknown output type: " + string(opt.OutputType)))
	}
}

func outputFormat(r Task) (content string, err error) {
	var outUrl string
	if opt.Raw {
		outUrl = r.RawUrl
	} else {
		outUrl = r.Url
	}
	fmt := opt.OutputFormat
	if fmt == "" {
		return outUrl, nil
	}
	val, ok := cfg.OutputFormats[fmt]
	if !ok {
		return "", errors.New("unknown output format: " + fmt)
	}
	content = strings.NewReplacer(
		"{url}", outUrl,
		"{urlfname}", filepath.Base(outUrl),
		"{fname}", filepath.Base(r.LocalPath),
	).Replace(RemoveFmtUnderscore(val))

	return
}

func validArgs() {
	if errs := validator.Validate(cfg); errs != nil {
		abortErr(fmt.Errorf("incorrect config: " + errs.Error()))
	}

	for _, path := range opt.LocalPaths {
		if strings.HasPrefix(path, "http") {
			continue
		}
		fs, err := os.Stat(path)
		if errors.Is(err, os.ErrNotExist) {
			abortErr(fmt.Errorf("invalid file to upload %s: no such file", path))
		}
		if err != nil {
			abortErr(fmt.Errorf("invalid file to upload %s: %s", path, err.Error()))
		}
		if fs.Size() == 0 {
			abortErr(fmt.Errorf("invalid file to upload %s: file size is zero", path))
		}
		if maxUploadSize != 0 && fs.Size() > maxUploadSize {
			abortErr(fmt.Errorf("invalid file to upload %s: file size is larger than %d bytes", path, maxUploadSize))
		}
	}
}

func loadTomlConfig(cfg *Config) {

	homeDir, err := os.UserHomeDir()
	panicErr(err)

	appDir, err := GetApplicationPath()
	panicErr(err)

	var configFiles = []string{
		filepath.Join(homeDir, ".upgit.config.toml"),
		filepath.Join(homeDir, ".upgit.toml"),
		filepath.Join(appDir, "config.toml"),
	}

	for _, configFile := range configFiles {
		if _, err := os.Stat(configFile); err != nil {
			continue
		}
		optRawBytes, err := ioutil.ReadFile(configFile)
		if err == nil {
			err = toml.Unmarshal(optRawBytes, &cfg)
		}
		if err != nil {
			abortErr(fmt.Errorf("invalid config: " + err.Error()))
		}
		configFilePath = configFile
		return
	}

}

type Nullable[T any] struct {
	Value *T
}

func loadUploaderConfig[T any](name string) (ret T, err error) {
	var mCfg map[string]interface{}
	bytes, err := ioutil.ReadFile(configFilePath)
	if err != nil {
		return
	}
	err = toml.Unmarshal(bytes, &mCfg)
	if err != nil {
		return
	}
	cfgMap := mCfg["uploaders"].(map[string]interface{})[name]
	var cfg_ T
	mapstructure.Decode(cfgMap, &cfg_)
	ret = cfg_
	return
}

func upload() {
	if opt.Uploader == "" {
		opt.Uploader = cfg.DefaultUploader
	}
	if opt.Uploader == "github" {
		// load GithubUploader config
		gCfg, err := loadUploaderConfig[GithubUploaderConfig](opt.Uploader)
		abortErr(err)
		if len(gCfg.Branch) == 0 {
			gCfg.Branch = kDefaultBranch
		}

		uploader := GithubUploader{Config: gCfg, OnUploaded: onUploaded}
		uploader.UploadAll(opt.LocalPaths, opt.TargetDir)
		return
	}
	// try http simple uploader
	// list file in ./extensions
	extDir := MustGetApplicationPath("extensions")
	info, err := ioutil.ReadDir(extDir)
	abortErr(err)
	var uploader *SimpleHttpUploader
	for _, f := range info {
		fname := f.Name()
		GVerbose.Trace("found file %s", fname)
		if !strings.HasSuffix(fname, ".json") && !strings.HasSuffix(fname, ".jsonc") {
			GVerbose.Trace("ignored file %s", fname)
			continue
		}
		// load file to json
		jsonBytes, err := ioutil.ReadFile(filepath.Join(extDir, fname))
		abortErr(err)
		jsonBytes = RemoveJsoncComments(jsonBytes)
		GVerbose.Trace("file content: %s", string(jsonBytes))
		var uploaderDef map[string]interface{}
		err = json.Unmarshal(jsonBytes, &uploaderDef)

		abortErr(err)
		if FromGoRet[string](xmap.GetDeep[string](uploaderDef, `meta.id`)).ValueOrExit() != opt.Uploader {
			continue
		}
		if FromGoRet[string](xmap.GetDeep[string](uploaderDef, "meta.type")).ValueOrExit() != "simple-http-uploader" {
			continue
		}
		uploader = &SimpleHttpUploader{OnUploaded: onUploaded, Definition: uploaderDef}
		extConfig, err := loadUploaderConfig[map[string]interface{}](opt.Uploader)
		if err == nil {
			uploader.Config = extConfig
			GVerbose.Trace("uploader config:")
			GVerbose.TraceStruct(uploader.Config)
		} else {
			GVerbose.Trace("no uploader config found")
		}
		break
	}
	if nil == uploader {
		abortErr(errors.New("unknown uploader: " + opt.Uploader))
	}
	uploader.UploadAll(opt.LocalPaths, opt.TargetDir)
	return

}


func loadClipboard() {
	if len(opt.LocalPaths) == 1 && strings.ToLower(opt.LocalPaths[0]) == kClipboardPlaceholder {
		err := clipboard.Init()
		if err != nil {
			abortErr(fmt.Errorf("failed to init clipboard: " + err.Error()))
		}

		tmpFileName := fmt.Sprint(os.TempDir(), "/upgit_tmp_", time.Now().UnixMicro(), ".png")
		buf := clipboard.Read(clipboard.FmtImage)
		if nil == buf {
			// try second chance for Windows user. To adapt bitmap format (compatible with Snipaste)
			if runtime.GOOS == "windows" {
				buf, err = xclipboard.ReadClipboardImage()
			}
			if err != nil {
				GVerbose.Error("failed to read clipboard image: " + err.Error())
			}
		}
		if nil == buf {
			abortErr(fmt.Errorf("failed: no image in clipboard or unsupported format"))
		}
		os.WriteFile(tmpFileName, buf, os.FileMode(fs.ModePerm))
		opt.LocalPaths[0] = tmpFileName
		opt.Clean = true
	}
}

func loadEnvConfig(cfg *Config) {
	if nil == cfg {
		abortErr(fmt.Errorf("unable to load env config: nil config"))
	}

	if rename, found := syscall.Getenv("UPGIT_RENAME"); found {
		cfg.Rename = rename
	}
}

func loadGithubUploaderEnvConfig(gCfg *GithubUploaderConfig) {
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
