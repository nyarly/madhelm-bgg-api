self-packages:
{
  lib,
  ...
}:
let
  inherit (lib) mkOption mkEnableOption;
  inherit (lib.types)
    str
    submodule
    port
    bool
    attrsOf
    listOf
    nullOr
    package
    int
    ;
in
{
  services.bgg-api = {
    enable = mkEnableOption { name = "bggapi"; };
    package = mkOption {
      type = package;
      default = self-packages.bgg-api;
    };
    migrationsPackage = mkOption {
      type = package;
      default = self-packages.bgg-api-migrations;
    };
    user = mkOption {
      type = str;
      default = "bggapi";
    };
    group = mkOption {
      type = str;
      default = "bggapi";
    };
    adminEmail = mkOption {
      type = str;
      example = "admin@wagthepig.com";
    };
    canonDomain = mkOption {
      type = str;
      example = "bggapi.wagthepig.com";
      default = "bggapi.wagthepig.com";
    };
    trustForwarded = mkOption {
      type = bool;
      description = ''
        The backend does rate limiting based on the IP of the requester.
        If you're running behind a reverse proxy (e.g. httpd or nginx),
        you should configure it to send Forwarded headers, and set this to true.
        If you're running it on its own, set this as false so that bad actors
        can't construct requests with Forwarded headers to evade rate limiting.
      '';
    };
    extraEnvironment = mkOption {
      type = attrsOf str;
      default = { };
    };
    listen = mkOption {
      description = "the address and port to listen on";

      default = { };
      type = submodule {
        options = {
          host = mkOption {
            type = str;
            default = "127.0.0.1";
          };
          port = mkOption {
            type = port;
            default = 3001;
          };
        };
      };
    };

    authMap = mkOption {
      description = "A mapping of our service domains to upstream authentication authorities";
      type = attrsOf str;
    };

    corsURLs = mkOption {
      description = "A list of CORS allowed Origin URLs";
      type = listOf str;
    };

    bggAPITokenPath = mkOption {
      description = "Path to the API token from https://boardgamegeek.com/applications";
      type = str;
    };

    bggSimultaneusRequests = mkOption {
      description = "How many simultaneus requests to attempt against the BGG XML API";
      type = int;
      default = 10;
    };

    database = mkOption {
      description = "Configuration for the required PostgreSQL database.";

      default = { };
      type = submodule {
        options = {
          user = mkOption {
            type = str;
            default = "bggapi";
          };
          host = mkOption {
            type = str;
            default = "localhost";
          };
          port = mkOption {
            type = port;
            default = 5432;
          };
          name = mkOption {
            type = str;
            default = "bggapi";
          };
          passwordPath = mkOption {
            type = nullOr str;
            default = null;
          };
        };
      };
    };
  };
}
