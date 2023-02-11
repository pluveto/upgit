package xapp

type OutputType string

const (
	O_Stdout    OutputType = "stdout"
	O_Clipboard            = "clipboard"
)

const kRepoURL = "https://github.com/pluveto/upgit"

type CLIOptions struct {
	LocalPaths   []string   `arg:"positional, required" placeholder:"FILE" help:"local file path to upload. :clipboard for uploading clipboard image"`
	TargetDir    string     `arg:"-t,--target-dir"    help:"upload file with original name to given directory. if not set, will use renaming rules"`
	Verbose      bool       `arg:"-V,--verbose"       help:"when set, output more details to help developers"`
	SizeLimit    *int64     `arg:"-s,--size-limit"    help:"in bytes. overwrite default size limit (5MiB). 0 means no limit"`
	ScaleFix     *int       `arg:"-S,--scale-fix"     help:"when set, will scale image to reciprocal of given value. 100 means no scale. you can set to your screen scale factor"`
	Wait         bool       `arg:"-w,--wait"          help:"when set, not exit after upload, util user press any key"`
	ConfigFile   string     `arg:"-c,--config-file"   help:"when set, will use specific config file"`
	Clean        bool       `arg:"-C,--clean"         help:"when set, remove local file after upload"`
	Raw          bool       `arg:"-r,--raw"           help:"when set, output non-replaced raw url"`
	NoLog        bool       `arg:"-n,--no-log"        help:"when set, disable logging"`
	Uploader     string     `arg:"-u,--uploader"      help:"uploader to use. if not set, will follow config"`
	OutputType   OutputType `arg:"-o,--output-type"   help:"output type, supports stdout, clipboard" default:"stdout"`
	OutputFormat string     `arg:"-f,--output-format" help:"output format, supports url, markdown and your customs" default:"url"`

	ApplicationPath string `arg:"--application-path" help:"custom application path, which determines config file path and extensions dir path. current binary dir by default"`
}

func (CLIOptions) Description() string {
	return "\n" +
		"Upload anything to github repo or other remote storages and then get its link.\n" +
		"For more information: " + kRepoURL + "\n"
}

var AppOpt CLIOptions
