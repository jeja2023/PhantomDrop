#!/bin/sh
set -eu

# Bind mounts replace image ownership; repair them before dropping privileges.
chown -R phantom:phantom /app/data /app/.automation

exec gosu phantom "$@"
