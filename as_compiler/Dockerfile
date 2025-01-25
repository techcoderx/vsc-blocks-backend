FROM node:22-alpine AS base
ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable
# RUN adduser --disabled-password --gecos '' vsc_as
COPY . /workdir
WORKDIR /workdir

# USER root
# RUN chown -R vsc_as:vsc_as /workdir
RUN chmod +x /workdir/entrypoint.sh

# USER vsc_as
ENTRYPOINT ["/workdir/entrypoint.sh"]