# A Linux desktop the Computer Use server can be qualified against.
#
# The X11 backend shells out to xdotool/wmctrl/scrot/xclip and reads the
# accessibility tree through AT-SPI (python3 + pyatspi over the session bus),
# and the parity suite needs its two fixtures: a Chrome --app page and a Tk
# window. This image supplies all of that plus Xvfb and a window manager, so
# `npm run parity -- --isolated` reproduces here with nothing installed on the
# host but Docker.
#
# It is a headless X11 session in a container: it qualifies the X11 code paths
# and the isolated parity route. It is not evidence for Wayland, for a real
# login session's window manager (see the KWin modal-dialog row in
# docs/LIMITATIONS.md), or for anything the native macOS app holds.
FROM node:24-bookworm-slim

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
      xvfb openbox x11-utils x11-xserver-utils xauth \
      xdotool wmctrl scrot xclip xsel \
      dbus-x11 at-spi2-core python3-pyatspi python3-gi gir1.2-gtk-3.0 \
      python3-tk \
      chromium fonts-dejavu-core ffmpeg \
      ca-certificates procps git \
  && rm -rf /var/lib/apt/lists/*

# Chromium's own sandbox needs privileges this container does not have. Debian's
# launcher sources /etc/chromium.d/*, so the flags apply without the parity
# driver knowing it is in a container.
RUN printf '%s\n' \
      'export CHROMIUM_FLAGS="$CHROMIUM_FLAGS --no-sandbox --disable-dev-shm-usage --disable-gpu"' \
      > /etc/chromium.d/00-container

# Toolkits load the AT-SPI bridge unless told not to; say so explicitly rather
# than relying on the default.
# xdotool refuses to type non-ASCII text under the C locale ("Invalid
# multi-byte sequence"), which is most of what the unicode tasks exist to check.
ENV LANG=C.UTF-8 \
    LC_ALL=C.UTF-8

ENV NO_AT_BRIDGE=0 \
    QT_ACCESSIBILITY=1 \
    GTK_A11Y=atspi \
    XDG_SESSION_TYPE=x11 \
    CODEWHALE_CU_APP=off

WORKDIR /app
RUN chown node:node /app
USER node

# Dependencies first so source edits do not re-resolve the tree. There are no
# runtime dependencies; this installs sharp for the icon scripts only.
COPY --chown=node:node package.json package-lock.json ./
RUN npm ci --ignore-scripts

COPY --chown=node:node . .

ENTRYPOINT ["/bin/sh", "/app/docker/entrypoint.sh"]
CMD ["npm", "test"]
