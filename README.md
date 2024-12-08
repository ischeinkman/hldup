A program to `h`ard `l`ink `dup`licate files. 


## Building 

To build the application, use `cargo build` from the root of the repo. Right now
there are no pre-built artifacts for this application, but this may change in
the future. 

## Usage 

By default, running a bare `hldup` command will look for any non-hardlinked
non-symlinked duplicates in the current working directory and then prompt before
hard-linking them. 

You can pass in the `--default-yes`, `--default-no`, or `--prompt` flags to
change the behaviour when a duplicate is encountered. `--prompt` is the default
behaviour, which asks the user on `stdin` whether or not the files should be
linked; `--default-yes` and `--default-no` act as if the user has already said
"yes" or "no" to that prompt, respectively. The `--default-no` behaviour is
intented for checking for duplicates on a filesystem without modifying that
filesystem.

You can pass one or more directories on the command line to check for
duplicates. If any directories are passed in then the current working directory
will not be automatically added. If multiple directories are passed, `hldup`
*will* also find the duplicates across directories, not just within the
directories in isolation.

## Debugging & Logging

The log level emitted by this program can be controlled with the `HLDUP_LOG`
environment variable; this defaults to `INFO`, but can be increased to `DEBUG`
or `TRACE` or decreased to `WARN` or `ERROR` if necessary. 