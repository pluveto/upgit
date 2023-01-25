DIST_DIR = ./dist
BINARY = $(DIST_DIR)/upgit
SRC = .
LDFLAGS = -ldflags="-s -w"

create_dist_dir: 
	@mkdir -p $(DIST_DIR)

windows: create_dist_dir
	GOOS=windows GOARCH=386     go build -o $(BINARY)_win_386.exe   $(LDFLAGS) $(SRC)
	GOOS=windows GOARCH=amd64   go build -o $(BINARY)_win_amd64.exe $(LDFLAGS) $(SRC)
	GOOS=windows GOARCH=arm     go build -o $(BINARY)_win_arm.exe   $(LDFLAGS) $(SRC)
	GOOS=windows GOARCH=arm64   go build -o $(BINARY)_win_arm64.exe $(LDFLAGS) $(SRC)

# Build agent should be run on macos, otherwise it will fail at macos target
# ```
# # runtime/cgo
# gcc: error: unrecognized command-line option '-arch'
# make: *** [Makefile:16: macos] Error 2
# ```
macos: create_dist_dir
	GOOS=darwin  GOARCH=amd64   CGO_ENABLED=1 go build -o $(BINARY)_macos_amd64   $(LDFLAGS) $(SRC)
	GOOS=darwin  GOARCH=arm64   CGO_ENABLED=1 go build -o $(BINARY)_macos_arm64   $(LDFLAGS) $(SRC)

# You need to install gcc-multilib to build 32-bit binaries on 64-bit Linux
# ```
# sudo apt-get install gcc-multilib
# ```
linux: create_dist_dir
	GOOS=linux   GOARCH=386     go build -o $(BINARY)_linux_386     $(LDFLAGS) $(SRC)
	GOOS=linux   GOARCH=amd64   go build -o $(BINARY)_linux_amd64   $(LDFLAGS) $(SRC)
	GOOS=linux   GOARCH=arm     go build -o $(BINARY)_linux_arm     $(LDFLAGS) $(SRC)
	GOOS=linux   GOARCH=arm64   go build -o $(BINARY)_linux_arm64   $(LDFLAGS) $(SRC)

	GOOS=linux   GOARCH=amd64   CGO_ENABLED=1 go build -o $(BINARY)_cgo_linux_amd64   $(LDFLAGS) $(SRC)

upx: all
	for i in $(DIST_DIR)/*
	do
		upx -9 $i
	done

all: windows macos linux
	@echo "done."

clean:
	rm -rfd $(DIST_DIR)
