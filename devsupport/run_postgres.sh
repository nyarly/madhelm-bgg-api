#! /usr/bin/env bash

set -e
devsupport=$(realpath $(dirname $0))
if [ ! -d "$PGDATA" ]; then
  initdb --no-instructions -U postgres
fi
exec postgres -c log_min_duration_statement=0 -c listen_addresses= -c unix_socket_directories="${devsupport}/db_sockets"
