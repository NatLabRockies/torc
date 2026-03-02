(function () {
  "use strict";

  // Determine the base path (e.g., "/torc/")
  var basePath = "/torc/";

  // Parse current version from URL path
  function getCurrentVersion() {
    var path = window.location.pathname;
    // Match /torc/vX.Y.Z/ or /torc/latest/
    var match = path.match(/^\/torc\/(v[\d.]+|latest)\//);
    return match ? match[1] : "latest";
  }

  // Get the relative page path within the versioned docs
  function getRelativePath() {
    var path = window.location.pathname;
    var match = path.match(/^\/torc\/(?:v[\d.]+|latest)\/(.*)/);
    return match ? match[1] : "";
  }

  function createSelector(versions, currentVersion) {
    var container = document.createElement("div");
    container.className = "version-selector";

    var select = document.createElement("select");
    select.id = "version-select";
    select.setAttribute("aria-label", "Select documentation version");

    versions.forEach(function (v) {
      var option = document.createElement("option");
      option.value = v.path;
      option.textContent = v.label;
      if (v.version === currentVersion) {
        option.selected = true;
      }
      select.appendChild(option);
    });

    select.addEventListener("change", function () {
      var relativePath = getRelativePath();
      window.location.href = select.value + relativePath;
    });

    container.appendChild(select);
    return container;
  }

  function init() {
    fetch(basePath + "versions.json")
      .then(function (response) {
        if (!response.ok) throw new Error("versions.json not found");
        return response.json();
      })
      .then(function (data) {
        var currentVersion = getCurrentVersion();
        var selector = createSelector(data.versions, currentVersion);

        var rightButtons = document.querySelector(".right-buttons");
        if (rightButtons) {
          rightButtons.insertBefore(selector, rightButtons.firstChild);
        }
      })
      .catch(function () {
        // Silently fail — expected in local dev without versions.json
      });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
