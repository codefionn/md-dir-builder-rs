/** @var HTMLElement */
const comp_content = document.body.querySelector("#contents");
/** @var HTMLElement */
const comp_sidebar = document.body.querySelector("#sidebar");

const wslink = "ws://" + document.location.host + "/.ws";
const socket = new WebSocket(wslink);
socket.onmessage = function(event) {
  const data = JSON.parse(event.data);
  console.debug(data);

  switch (data.action) {
  case "update-content":
    const current_path = document.location.pathname.split("/")
                             .map(part => decodeURI(part))
                             .join("/");
    if (current_path === data.path) {
      comp_content.innerHTML = data.content;
      if (typeof window.Prism === "object") {
        window.Prism.highlightAllUnder(comp_content);
      }
    }
    break;
  case "update-sidebar":
    comp_sidebar.innerHTML = data.content;
    break;
  }
};

function fetch_contents(pathname, successfn) {
  fetch("/.contents" + pathname)
      .then(response => response.text())
      .then(contents => {
        comp_content.innerHTML = contents;
        if (typeof window.Prism === "object") {
          window.Prism.highlightAllUnder(comp_content);
        }

        if (successfn) {
          successfn();
        }
      });
}

window.onpopstate = (event) => {
  const href = document.location.pathname;
  if (href.startsWith("/.")) {
  } else {
    fetch_contents(href);
  }
};

const comp_files = document.body.querySelectorAll("#sidebar .file a");
comp_files.forEach(
    comp_file => {comp_file.addEventListener("click", (event) => {
      event.preventDefault();

      const url = new URL(comp_file.href);

      fetch_contents(
          url.pathname,
          () => { history.pushState({}, url.pathname, url.pathname); });
    })});
