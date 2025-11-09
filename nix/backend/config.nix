{
  config,
  lib,
  pkgs,
  ...
}:
lib.mkIf config.services.bgg-api.enable (
  let
    cfg = config.services.bgg-api;
    package = cfg.package;
    maybeSMTPCert =
      if cfg.smtp.certPath != null then
        {
          SMTP_CERT = cfg.smtp.certPath;
        }
      else
        { };
    dbPass = if cfg.database.passwordPath == null then "" else ":$(cat ${cfg.database.passwordPath})";

    dbURL = "export DATABASE_URL=postgres://${cfg.database.user}${dbPass}@${cfg.database.host}:${toString cfg.database.port}/${cfg.database.name}";
  in
  {
    users = {
      users = {
        "${cfg.user}" = {
          name = cfg.user;
          group = cfg.group;
          extraGroups = [ "keys" ];
          isSystemUser = true;
        };
      };

      groups = {
        "${cfg.group}" = { };
      };
    };

    systemd.services.bgg-api =
      let
        authMap = builtins.concatStringSep "," (lib.mapAttrsToList (n: v: "${n}=${v}") cfg.authMap);
        corsOrigins = builtins.concatStringsSep "," cfg.corsURLs;
        preStart =
          (pkgs.writeShellScriptBin "bggapi-prestart"
            #bash
            ''
              set -e

              ${pkgs.postgresql}/bin/psql -h ${cfg.database.host} -p ${toString cfg.database.port} -U postgres <<'SQL'
              do $$
              begin
                create role ${cfg.database.user};
                exception when duplicate_object then raise notice '%, skipping', sqlerrm using errcode = SQLSTATE;
              end
              $$;
              SQL

              ${pkgs.postgresql}/bin/psql -h ${cfg.database.host} -p ${toString cfg.database.port} -U postgres <<'SQL' || echo "already exists"
              create database ${cfg.database.name} with owner ${cfg.database.user};
              SQL

              ${dbURL}
              ${pkgs.sqlx-cli}/bin/sqlx migrate run --source ${package.migrations}
            ''
          ).overrideAttrs
            (_: {
              name = "unit-script-bggapi-prestart";
            });
      in
      {
        after = [
          "network.target"
          "postgresql.service"
        ];
        wants = [ ];

        wantedBy = [ "multi-user.target" ];

        serviceConfig = {
          StateDirectory = "bgg-api";
          User = cfg.user;
          Group = cfg.group;
          ExecStartPre = [ "!${lib.getExe preStart}" ];
        };

        environment = {
          LOCAL_ADDR = "${cfg.listen.host}:${toString cfg.listen.port}";
          CANON_DOMAIN = cfg.canonDomain;
          # DATABASE_URL provided by start script via SOPS
          TRUST_FORWARDED_HEADER = lib.boolToString cfg.trustForwarded;
          # BGG_API_TOKEN provided by start script via SOPS
          BGG_SIMULTANEUS_REQUESTS = cfg.bggSimultaneusRequests;
          AUTH_MAP = authMap;
          CORS_ORIGINS = corsOrigins;
        }
        // cfg.extraEnvironment
        // maybeSMTPCert;

        script = ''
          ${dbURL}
          export BGG_API_TOKEN=$(cat ${cfg.bggAPITokenPath})
          ${package}/bin/bggapi-backend
        '';
      };
  }
)
