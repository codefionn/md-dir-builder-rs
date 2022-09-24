const comp_content = document.body.querySelector("#contents");

const wslink = "ws://" + document.location.host + "/.ws";
const socket = new WebSocket(wslink);
socket.onmessage = function(event) {
  const data = JSON.parse(event.data);
  console.debug(data);

  switch (data.action) {
  case "update-content":
    comp_content.innerHTML = data.content;
    break;
  }
};
