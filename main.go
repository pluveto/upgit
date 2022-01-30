package main

import (
	"errors"
	"fmt"
	"io/fs"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"time"

	"github.com/alexflint/go-arg"
	"github.com/pelletier/go-toml/v2"
	"golang.design/x/clipboard"
	"gopkg.in/validator.v2"
)

type OutputType string

const (
	O_Stdout            OutputType = "stdout"
	O_Clipboard                    = "clipboard"
	O_ClipboardMarkdown            = "clipboard-markdown"
)

const kDefaultBranch = "master"
const kClipboardPlaceholder = ":clipboard"
const kRepoURL = "https://github.com/pluveto/upgit"

var maxUploadSize = int64(5 * 1024 * 1024)

type CLIOptions struct {
	LocalPaths []string   `arg:"positional, required" placeholder:"FILE" help:"local file path to upload. :clipboard for uploading clipboard image"`
	TargetDir  string     `arg:"-t,--target-dir"  help:"upload file with original name to given directory. if not set, will use renaming rules"`
	Verbose    bool       `arg:"-V,--verbose"     help:"when set, output more details to help developers"`
	SizeLimit  *int64     `arg:"-s,--size-limit"  help:"in bytes. overwrite default size limit (5MiB). 0 means no limit"`
	Wait       bool       `arg:"-w,--wait"        help:"when set, not exit after upload, util user press any key"`
	Clean      bool       `arg:"-c,--clean"       help:"when set, remove local file after upload"`
	Raw        bool       `arg:"-r,--raw"         help:"when set, output non-replaced raw url"`
	OutputType OutputType `arg:"-o,--output-type" help:"output type, supports stdout, clipboard, clipboard-markdown" default:"stdout"`
}

func (CLIOptions) Description() string {
	return "\n" +
		"Upload anything to github repo and then get its link.\n" +
		"For more information: " + kRepoURL + "\n"
}

type Config struct {
	PAT          string            `toml:"pat" validate:"nonzero"`
	Rename       string            `toml:"rename,omitempty"`
	Replacements map[string]string `toml:"replacements,omitempty"`
	Username     string            `toml:"username" validate:"nonzero"`
	Repo         string            `toml:"repo" validate:"nonzero"`
	Branch       string            `toml:"branch,omitempty"`
}

var opt CLIOptions
var cfg Config = Config{Branch: kDefaultBranch}

func main() {

	// parse cli args
	arg.MustParse(&opt)
	opt.TargetDir = strings.Trim(opt.TargetDir, "/")

	if opt.SizeLimit != nil && *opt.SizeLimit >= 0 {
		maxUploadSize = *opt.SizeLimit
	}
	GVerbose.Enabled = opt.Verbose
	GVerbose.TraceStruct(opt)

	// load config
	loadEnvConfig(&cfg)
	loadTomlConfig(&cfg)
	cfg.Rename = strings.Trim(cfg.Rename, "/")
	cfg.Rename = RemoveFmtUnderscore(cfg.Rename)
	GVerbose.TraceStruct(cfg)

	// handle clipboard
	loadClipboard(&opt)

	// validating args
	validArgs(cfg, opt)

	// executing uploading
	uploader := GithubUploader{Config: cfg, OnUploaded: onUploaded}
	uploader.UploadAll(opt.LocalPaths, opt.TargetDir)

	if opt.Wait {
		fmt.Scanln()
	}

	return
}

func onUploaded(r Result[UploadRet]) {
	if !r.Ok() && opt.OutputType == O_Stdout {
		fmt.Println("Failed: " + r.err.Error())
		return
	}
	if opt.Clean && r.value.Status != Ignored {
		_ = os.Remove(r.value.LocalPath)
	}

	outputLink(r.value)
}

func outputLink(r UploadRet) {
	var outUrl string
	if opt.Raw {
		outUrl = r.RawUrl
	} else {
		outUrl = r.Url
	}
	switch opt.OutputType {
	case O_Stdout:
		fmt.Println(outUrl)
	case O_Clipboard:
		clipboard.Write(clipboard.FmtText, []byte(outUrl))
	case O_ClipboardMarkdown:
		clipboard.Write(clipboard.FmtText, []byte(
			fmt.Sprint("![", filepath.Base(outUrl), "](", outUrl, ")"),
		))
	}
}

func validArgs(cfg Config, opt CLIOptions) {
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
		return
	}

}

func loadClipboard(opt *CLIOptions) {
	if len(opt.LocalPaths) == 1 && strings.ToLower(opt.LocalPaths[0]) == kClipboardPlaceholder {
		err := clipboard.Init()
		if err != nil {
			abortErr(fmt.Errorf("failed to init clipboard: " + err.Error()))
		}

		tmpFileName := fmt.Sprint(os.TempDir(), "/upgit_tmp_", time.Now().UnixMicro(), ".png")
		buf := clipboard.Read(clipboard.FmtImage)
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

	if pat, found := syscall.Getenv("GITHUB_TOKEN"); found {
		cfg.PAT = pat
	}
	if pat, found := syscall.Getenv("UPGIT_TOKEN"); found {
		cfg.PAT = pat
	}
	if rename, found := syscall.Getenv("UPGIT_RENAME"); found {
		cfg.Rename = rename
	}
	if username, found := syscall.Getenv("UPGIT_USERNAME"); found {
		cfg.Username = username
	}
	if repo, found := syscall.Getenv("UPGIT_REPO"); found {
		cfg.Repo = repo
	}
	if branch, found := syscall.Getenv("UPGIT_BRANCH"); found {
		cfg.Branch = branch
	}
}
