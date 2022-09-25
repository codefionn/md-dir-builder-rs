const comp_content = document.body.querySelector("#contents");

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
    }
    break;
  }
};
