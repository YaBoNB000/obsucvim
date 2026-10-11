"""Linux resource-limited launcher. Internal arguments only, not an HTTP API."""
import os
import resource
import sys


def main():
    memory_mb, output_bytes, cpu_seconds = map(int, sys.argv[1:4])
    resource.setrlimit(resource.RLIMIT_AS, (memory_mb * 1024 * 1024,) * 2)
    resource.setrlimit(resource.RLIMIT_FSIZE, (output_bytes,) * 2)
    resource.setrlimit(resource.RLIMIT_CPU, (cpu_seconds, cpu_seconds + 1))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    os.umask(0o077)
    os.execv(sys.argv[4], sys.argv[4:])


if __name__ == "__main__":
    main()
