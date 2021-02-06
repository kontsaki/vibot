FROM redislabs/redisjson2 as redisjson
FROM rust:latest

COPY --from=redisjson /usr/lib/redis/modules/libredisjson.so /usr/lib/redis/modulus/rejson.so
RUN apt-get update && apt-get install -y redis-server entr fd-find git build-essential libssl-dev pkg-config

CMD redis-server --loadmodule /usr/lib/redis/modulus/rejson.so & bash
