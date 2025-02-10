BINARY = lazaro

DB_FILE = lazaro.db

.PHONY: all build run clean test release

all: build

build:
	@go build -o $(BINARY) .

clean:
	@rm -f $(BINARY) $(DB_FILE)

test:
	@go test ./...
