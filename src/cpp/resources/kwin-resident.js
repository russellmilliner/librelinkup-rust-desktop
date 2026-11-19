var dbusAddr = '__DBUS_ADDR__';
var dbusIface = '__DBUS_IFACE__';
var targetOvX = __TARGET_OV_X__;
var targetOvY = __TARGET_OV_Y__;
var targetOvW = __TARGET_OV_W__;
var targetOvH = __TARGET_OV_H__;
var activeWindows = {};
var captionHookedIds = {};
var hookedOverlayIds = {};
var hookedOverlayTime = {};
var applyingOverlayTime = {};
var hookedMainIds = {};
var movingOverlayIds = {};
var movingMainIds = {};
function windowListAll() {
  return workspace.stackingOrder || workspace.windowList();
}
function isLibreLinkUpCandidate(w) {
  if (!w) return false;
  var rc = w.resourceClass ? w.resourceClass.toString().toLowerCase() : '';
  var df = w.desktopFileName ? w.desktopFileName.toString().toLowerCase() : '';
  return rc.indexOf('librelinkup') !== -1 || df.indexOf('librelinkup') !== -1;
}
function isOverlayWin(w) {
  if (!isLibreLinkUpCandidate(w)) return false;
  var cap = w.caption ? w.caption.toString() : '';
  return cap.indexOf('LibreLinkUp Rust Desktop Overlay') === 0 && cap.indexOf('Tooltip') === -1;
}
function isMainWin(w) {
  if (!isLibreLinkUpCandidate(w)) return false;
  var cap = w.caption ? w.caption.toString() : '';
  return cap.indexOf('LibreLinkUp Rust Desktop v') === 0;
}
function windowKey(w) {
  return (w && w.internalId) ? w.internalId.toString() : '';
}
function applyOverlayFrame(w, wid, gx, gy, gw, gh) {
  if (!w || w.move || w.resize || gw <= 0 || gh <= 0 || isNaN(gx) || isNaN(gy)) return;
  if (wid) applyingOverlayTime[wid] = Date.now();
  w.frameGeometry = { x: gx, y: gy, width: gw, height: gh };
}
function reportOverlayGeometry(w) {
  if (!w || w.move || w.resize) return;
  var outName = (w.output && w.output.name) ? w.output.name.toString() : '';
  var fg = w.frameGeometry;
  if (dbusAddr && fg && fg.width > 0 && fg.height > 0) {
    targetOvX = Math.round(fg.x);
    targetOvY = Math.round(fg.y);
    targetOvW = Math.round(fg.width);
    targetOvH = Math.round(fg.height);
    callDBus(dbusAddr, '/LibreLinkUpKWin', dbusIface, 'onKWinOverlayGeometry',
      outName, targetOvX, targetOvY, targetOvW, targetOvH);
  }
}
function reportMainGeometry(w) {
  if (!w || w.move || w.resize) return;
  var outName = (w.output && w.output.name) ? w.output.name.toString() : '';
  var fg = w.frameGeometry;
  if (dbusAddr && fg && fg.width > 0 && fg.height > 0) {
    callDBus(dbusAddr, '/LibreLinkUpKWin', dbusIface, 'onKWinMainWindowGeometry',
      outName, Math.round(fg.x), Math.round(fg.y), Math.round(fg.width), Math.round(fg.height));
  }
}
function enforceLibreLinkUpRules(w) {
  if (!w || !isLibreLinkUpCandidate(w)) return;
  var wid = windowKey(w);
  var cap = w.caption ? w.caption.toString() : '';
  if (wid && !captionHookedIds[wid]) {
    captionHookedIds[wid] = true;
    if (w.captionChanged) {
      w.captionChanged.connect(function() {
        enforceLibreLinkUpRules(w);
      });
    }
    if (w.closed) {
      w.closed.connect(function() {
        delete captionHookedIds[wid];
        delete activeWindows[wid];
        delete hookedOverlayIds[wid];
        delete hookedOverlayTime[wid];
        delete applyingOverlayTime[wid];
        delete hookedMainIds[wid];
        delete movingOverlayIds[wid];
        delete movingMainIds[wid];
      });
    }
  }
  if (cap.length === 0) {
    var fg0 = w.frameGeometry;
    if (!w.popupWindow && !w.tooltip && !w.transient
        && fg0 && fg0.width >= 220 && fg0.width <= 620 && fg0.height >= 72 && fg0.height <= 280) {
      if (!w.noBorder) w.noBorder = true;
      if (!w.keepAbove) w.keepAbove = true;
      if (!w.skipTaskbar) w.skipTaskbar = true;
      if (!w.skipPager) w.skipPager = true;
      if (!w.skipSwitcher) w.skipSwitcher = true;
      applyOverlayFrame(w, wid, targetOvX, targetOvY, targetOvW, targetOvH);
    }
    return;
  }
  if (isOverlayWin(w)) {
    var cap = w.caption ? w.caption.toString() : '';
    var allDesk = (cap.indexOf('[SingleDesktop]') === -1);
    if (!w.noBorder) w.noBorder = true;
    if (!w.keepAbove) w.keepAbove = true;
    if (!w.skipTaskbar) w.skipTaskbar = true;
    if (!w.skipPager) w.skipPager = true;
    if (!w.skipSwitcher) w.skipSwitcher = true;
    if (w.onAllDesktops !== allDesk) w.onAllDesktops = allDesk;
    if (w.minimized) {
      w.minimized = false;
    }
    if (wid && !hookedOverlayIds[wid]) {
      hookedOverlayIds[wid] = true;
      hookedOverlayTime[wid] = Date.now();
      activeWindows[wid] = w;
      applyOverlayFrame(w, wid, targetOvX, targetOvY, targetOvW, targetOvH);
      if (w.skipTaskbarChanged) {
        w.skipTaskbarChanged.connect(function() {
          if (!w.skipTaskbar) w.skipTaskbar = true;
        });
      }
      if (w.skipPagerChanged) {
        w.skipPagerChanged.connect(function() {
          if (!w.skipPager) w.skipPager = true;
        });
      }
      if (w.skipSwitcherChanged) {
        w.skipSwitcherChanged.connect(function() {
          if (!w.skipSwitcher) w.skipSwitcher = true;
        });
      }
      if (w.keepAboveChanged) {
        w.keepAboveChanged.connect(function() {
          if (!w.keepAbove) w.keepAbove = true;
        });
      }
      if (w.noBorderChanged) {
        w.noBorderChanged.connect(function() {
          if (!w.noBorder) w.noBorder = true;
        });
      }
      if (w.minimizedChanged) {
        w.minimizedChanged.connect(function() {
          if (w.minimized) {
            w.minimized = false;
            if (!w.keepAbove) w.keepAbove = true;
            if (!w.skipTaskbar) w.skipTaskbar = true;
          }
        });
      }
      if (w.interactiveMoveResizeFinished) {
        w.interactiveMoveResizeFinished.connect(function() {
          delete movingOverlayIds[wid];
          reportOverlayGeometry(w);
        });
      }
      if (w.moveResizedChanged) {
        w.moveResizedChanged.connect(function() {
          if (w.move || w.resize) {
            movingOverlayIds[wid] = true;
          } else if (movingOverlayIds[wid]) {
            delete movingOverlayIds[wid];
            reportOverlayGeometry(w);
          }
        });
      }
      if (w.frameGeometryChanged) {
        w.frameGeometryChanged.connect(function() {
          if (w.move || w.resize || movingOverlayIds[wid]) return;
          var now = Date.now();
          if (now - (hookedOverlayTime[wid] || 0) < 450) return;
          if (now - (applyingOverlayTime[wid] || 0) < 180) return;
          var fg = w.frameGeometry;
          if (!fg) return;
          if (Math.abs(Math.round(fg.x) - targetOvX) > 2
              || Math.abs(Math.round(fg.y) - targetOvY) > 2
              || Math.abs(Math.round(fg.width) - targetOvW) > 2
              || Math.abs(Math.round(fg.height) - targetOvH) > 2) {
            reportOverlayGeometry(w);
          }
        });
      }
    }
    return;
  }
  if (isMainWin(w)) {
    if (w.minimized) {
      if (!w.skipTaskbar) w.skipTaskbar = true;
      if (!w.skipPager) w.skipPager = true;
      if (!w.skipSwitcher) w.skipSwitcher = true;
    } else {
      if (w.skipTaskbar) w.skipTaskbar = false;
      if (w.skipPager) w.skipPager = false;
      if (w.skipSwitcher) w.skipSwitcher = false;
    }
    if (wid && !hookedMainIds[wid]) {
      hookedMainIds[wid] = true;
      activeWindows[wid] = w;
      if (w.minimizedChanged) {
        w.minimizedChanged.connect(function() {
          if (w.minimized) {
            if (!w.skipTaskbar) w.skipTaskbar = true;
            if (!w.skipPager) w.skipPager = true;
            if (!w.skipSwitcher) w.skipSwitcher = true;
          } else {
            if (w.skipTaskbar) w.skipTaskbar = false;
            if (w.skipPager) w.skipPager = false;
            if (w.skipSwitcher) w.skipSwitcher = false;
          }
        });
      }
      if (w.interactiveMoveResizeFinished) {
        w.interactiveMoveResizeFinished.connect(function() {
          delete movingMainIds[wid];
          reportMainGeometry(w);
        });
      }
      if (w.moveResizedChanged) {
        w.moveResizedChanged.connect(function() {
          if (w.move || w.resize) {
            movingMainIds[wid] = true;
          } else if (movingMainIds[wid]) {
            delete movingMainIds[wid];
            reportMainGeometry(w);
          }
        });
      }
    }
  }
}
var list = windowListAll();
if (list) {
  for (var i = 0; i < list.length; ++i) {
    enforceLibreLinkUpRules(list[i]);
  }
}
if (workspace.windowAdded) {
  workspace.windowAdded.connect(function(w) {
    enforceLibreLinkUpRules(w);
  });
}
registerShortcut('LibreLinkUp SyncOverlay', 'LibreLinkUp (internal): Sync Overlay Window', '', function() {
  if (!dbusAddr) return;
  callDBus(dbusAddr, '/LibreLinkUpKWin', dbusIface, 'getDesiredOverlaySync', function(spec) {
    if (!spec) return;
    var parts = spec.toString().split(',');
    if (parts.length < 5) return;
    var gx = parseInt(parts[0], 10);
    var gy = parseInt(parts[1], 10);
    var gw = parseInt(parts[2], 10);
    var gh = parseInt(parts[3], 10);
    var allDesk = (parts[4] === '1' || parts[4] === 'true');
    if (gw > 0 && gh > 0 && !isNaN(gx) && !isNaN(gy)) {
      targetOvX = gx;
      targetOvY = gy;
      targetOvW = gw;
      targetOvH = gh;
    }
    var wl = windowListAll();
    if (!wl) return;
    for (var i = 0; i < wl.length; ++i) {
      var w = wl[i];
      if (isOverlayWin(w)) {
        var wid = windowKey(w);
        if (!w.noBorder) w.noBorder = true;
        if (w.onAllDesktops !== allDesk) w.onAllDesktops = allDesk;
        if (!w.keepAbove) w.keepAbove = true;
        if (!w.skipTaskbar) w.skipTaskbar = true;
        if (!w.skipPager) w.skipPager = true;
        if (!w.skipSwitcher) w.skipSwitcher = true;
        if (w.minimized) w.minimized = false;
        applyOverlayFrame(w, wid, gx, gy, gw, gh);
      }
    }
  });
});
registerShortcut('LibreLinkUp SyncMain', 'LibreLinkUp (internal): Sync Main Window', '', function() {
  if (!dbusAddr) return;
  callDBus(dbusAddr, '/LibreLinkUpKWin', dbusIface, 'getDesiredMainSync', function(spec) {
    if (!spec) return;
    var parts = spec.toString().split(',');
    if (parts.length < 4) return;
    var gx = parseInt(parts[0], 10);
    var gy = parseInt(parts[1], 10);
    var gw = parseInt(parts[2], 10);
    var gh = parseInt(parts[3], 10);
    var wl = windowListAll();
    if (!wl) return;
    for (var i = 0; i < wl.length; ++i) {
      var w = wl[i];
      if (isMainWin(w)) {
        if (!w.move && !w.resize && gw > 0 && gh > 0 && !isNaN(gx) && !isNaN(gy)) {
          w.frameGeometry = { x: gx, y: gy, width: gw, height: gh };
        }
        workspace.activeWindow = w;
      }
    }
  });
});
registerShortcut('LibreLinkUp QueryOverlay', 'LibreLinkUp (internal): Query Overlay Geometry', '', function() {
  var wl = windowListAll();
  if (!wl) return;
  for (var i = 0; i < wl.length; ++i) {
    var w = wl[i];
    if (isOverlayWin(w)) {
      reportOverlayGeometry(w);
    }
  }
});
registerShortcut('LibreLinkUp QueryMain', 'LibreLinkUp (internal): Query Main Geometry', '', function() {
  var wl = windowListAll();
  if (!wl) return;
  for (var i = 0; i < wl.length; ++i) {
    var w = wl[i];
    if (isMainWin(w)) {
      reportMainGeometry(w);
    }
  }
});
