-- Add migration script here

create table bgg_thing (
    id integer primary key generated always as identity,
    created_at timestamp with time zone not null default now(),
    updated_at timestamp with time zone not null default now(),
    retreived_at timestamp with time zone not null default now(),

    bgg_id text not null unique,
    kind text not null,
    name text,
    description text,
    thumbnail text,
    image text,
    year_published integer,
    min_players integer,
    max_players integer,
    min_duration integer,
    max_duration integer,
    duration integer
);

create table bgg_altname (
    id integer primary key generated always as identity,

    name text,
    thing_id integer references bgg_thing(id),
    unique (name, thing_id)
);

create type link as (
    bgg_id text,
    name text
);

create table bgg_category (
    id integer primary key generated always as identity,
    created_at timestamp with time zone not null default now(),

    bgg_id text not null unique,
    name text not null
);

create table thing_category (
    thing_id integer not null references bgg_thing(id) on delete cascade,
    category_id integer not null references bgg_category(id) on delete cascade
);

create table bgg_family (
    id integer primary key generated always as identity,
    created_at timestamp with time zone not null default now(),

    bgg_id text not null unique,
    name text not null
);

create table thing_family (
    thing_id integer not null references bgg_thing(id) on delete cascade,
    family_id integer not null references bgg_family(id) on delete cascade
);

create table bgg_designer (
    id integer primary key generated always as identity,
    created_at timestamp with time zone not null default now(),

    bgg_id text not null unique,
    name text not null
);

create table thing_designer (
    thing_id integer not null references bgg_thing(id) on delete cascade,
    designer_id integer not null references bgg_designer(id) on delete cascade
);

create table bgg_publisher (
    id integer primary key generated always as identity,
    created_at timestamp with time zone not null default now(),

    bgg_id text not null unique,
    name text not null
);

create table thing_publisher (
    thing_id integer not null references bgg_thing(id) on delete cascade,
    publisher_id integer not null references bgg_publisher(id) on delete cascade
);
