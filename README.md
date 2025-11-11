# The Madhelm BGG API Proxy

This is an API gateway server for the boardgamegeek.com XML API 2.

It serves several purposes:
1. It conceals the required application bearer token to access the BGG API
2. It authenticates use of the gateway to control for rate limits
3. It translates the XML responses of the BGG API into JSON
4. It caches data received from the API, as I understand it from the BGG usage guides.
5. It shares that caching, so that several apps can use the gateway and reduce their impact.

If that's of interest to you, read on for the caveats.

## Limited Implementation

So far,
I've built out the endpoints that are of interest to me.
That's meant the `search` and `thing` endpoints.
I will likely want to get the `geeklist` from
the old v1 XML API,
and maybe a few others.
At present, forum threads hold little interest.

Adding endpoints isn't terribly difficult,
and I might be willing to do that,
or accept PRs with new endpoints added.

## Authentication

Requests are authenticated by a
[biscuit](https://www.biscuitsec.org/)
that contains a user assertion.
Any user will do,
but the biscuit needs to be issued by a trusted authority.
In other words, this gateway is intended to be used
as a part of an ecosystem of apps authenticated by biscuits.
The upstream authorizer has to expose its public key
by way of Biscuit Well-known Key Sets,
and the API has a configuration for mapping
the Host headers on
requests to itself
to upstream authorities.

If all of that sounds like gobbledeygook,
one of two things are true:
this project might not be of use to you,
or you've just discovered a brand new
computer security rabbithole
to dive into.

## License
The
[license](./LICENSE.md)
for this project is
the MPL-2.0 license.
In addition,
please **let me know** that you're using it.
The long and the short of it
is that I just want to know if what I've been doing
is useful to someone else.

## Configuration and Deployment

If you can compile the server application in the first place,
simply running at the command line with a `--help` flag
will describe its configuration flags
and the environment variables it'll pull from without them.

For further explainations,
please reach out.
I'd be happy to expand this documentation
given that there's an audience.
