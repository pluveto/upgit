package main

import (
	"errors"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"
	"syscall"

	"github.com/alexflint/go-arg"
	"github.com/pelletier/go-toml/v2"
	"gopkg.in/validator.v2"
)

const kDefaultBranch = "master"
const kMaxUploadSize = int64(5 * 1024 * 1024)

type CLIOptions struct {
	LocalPaths []string `arg:"positional, required" placeholder:"FILE"`
	RemoteDir  string   `arg:"-d, --remote-dir"`
	Verbose    bool     `arg:"-V,--verbose" help:"verbosity level"`
}

func (CLIOptions) Description() string {
	return "\n" +
		"Upload anything to git and then get its link.\n" +
		"For more information: https://github.com/pluveto/upgit\n"
}

type Config struct {
	PAT          string            `toml:"pat" validate:"nonzero"`
	Rename       string            `toml:"rename,omitempty"`
	Replacements map[string]string `toml:"replacements,omitempty"`
	Username     string            `toml:"username" validate:"nonzero"`
	Repo         string            `toml:"repo" validate:"nonzero"`
	Branch       string            `toml:"branch,omitempty"`
}

func main() {

	// parse cli args
	var opt CLIOptions
	arg.MustParse(&opt)

	GVerbose.Enabled = opt.Verbose
	GVerbose.TraceStruct(opt)

	var cfg Config = Config{Branch: kDefaultBranch}
	loadEnvConfig(&cfg)

	// load config
	dir, err := GetApplicationPath()
	panicErr(err)
	optRawBytes, err := ioutil.ReadFile(filepath.Join(dir, "config.toml"))
	if err == nil {
		err = toml.Unmarshal(optRawBytes, &cfg)
	}
	if err != nil {
		abortErr(fmt.Errorf("invalid config: " + err.Error()))
	}
	cfg.Rename = strings.TrimLeft(cfg.Rename, "/")
	GVerbose.TraceStruct(cfg)

	// validating args
	validArgs(cfg, opt)

	// executing uploading
	uploader := GithubUploader{Options: cfg, OnUploaded: OnUploaded}
	uploader.UploadAll(opt.LocalPaths)
	return
}

func OnUploaded(r Result[UploadRet]) {
	if !r.Ok() {
		fmt.Println("Failed: " + r.err.Error())
		return
	}
	fmt.Println(r.value.Url)
}

func validArgs(cfg Config, opt CLIOptions) {
	if errs := validator.Validate(cfg); errs != nil {
		abortErr(fmt.Errorf("incorrect config: " + errs.Error()))
	}

	for _, path := range opt.LocalPaths {
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
		if fs.Size() > kMaxUploadSize {
			abortErr(fmt.Errorf("invalid file to upload %s: file size is larger than %d bytes", path, kMaxUploadSize))
		}
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
