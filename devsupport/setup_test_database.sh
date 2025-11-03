#!/usr/bin/env bash

root=$(realpath $(dirname $0)/..)
db_socket_path=$(echo $(pwd)/devsupport/db_sockets)

set -x
echo $root

psql -h $db_socket_path postgres << SQL
  CREATE DATABASE bggapi_empty_template WITH owner bggapi;
  CREATE DATABASE bggapi_seeded_template WITH owner bggapi;
SQL
sqlx migrate run -D $(echo $DATABASE_URL | sed 's/bggapi/bggapi_empty_template/') --source $root/backend/migrations
sqlx migrate run -D $(echo $DATABASE_URL | sed 's/bggapi/bggapi_seeded_template/') --source $root/backend/migrations
psql -h $db_socket_path bggapi_seeded_template < $root/devsupport/test_seeds.sql
