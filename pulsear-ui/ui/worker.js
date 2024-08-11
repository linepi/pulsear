// global value for this worker
let data = {
  id: null,
  builded: null,
  wsUri: null,
  resources_prefix: null,
  socket: null,
};

function start(id) {
  data.id = id;

  let socket = new WebSocket(data.wsUri);

  socket.onopen = evt => {
    let msg = new WsMessage(
      WsSender.withUser("", ""),
      WsMessageClass.withCreateWsWorker(id),
      WsDispatchType.Server
    );
    socket.send(msg.toJson());
    postMessage('SEND CREATEWSWORKER ' + msg.toJson());
  }

  socket.onmessage = evt => {
    let ws_message = WsMessage.fromJson(evt.data);
    if (ws_message.msg.is(WsMessageClass.CreateWsWorker)) {
      let id_from_server = ws_message.msg.content;
      if (data.id != id_from_server) {
        console.error("internal error!");
      }
      data.builded = true;
      postMessage('builded');
      postMessage('RECV CREATEWSWORKER ' + ws_message.toJson());
    }
  }

  socket.onclose = evt => {
    socket.onclose = null;
    socket = null;
    data.builded = false;
    postMessage('disconnect');
    postMessage('CLOSE');
  }

  socket.onerror = evt => {
    socket.onclose = null;
    socket = null;
    data.builded = false;
    postMessage('disconnect');
    postMessage('ERROR');
  }

  data.socket = socket;
}

function load(wsUri, resources_prefix) {
  data.wsUri = wsUri;
  data.resources_prefix = resources_prefix;
  importScripts(`${resources_prefix}ws.js`, `${resources_prefix}uploader.js`);
}

function handleCommand(cmd) {
  let args = cmd.split(' ');
  if (args[0] === 'start') {
    start(parseInt(args[1]));
    return;
  } else if (args[0] === 'reconnect') {
    start(data.id);
  } else if (args[0] === 'wbclose') {
    data.socket.close();
  } else if (args[0] === 'load') {
    load(args[1], args[2]);
  }
}

/**
  file: { 
    f: file, 
    req: request, 
    tr: null,
    name_td: null,
    name_overlay: null
  };
*/

function uploadSlice(file, hashval, slice_index, start, end) {
  if (data.socket != null && data.socket.readyState != WebSocket.OPEN) { 
    return;
  }
  const hashvalBlob = new Uint8Array(hashval.match(/[\da-f]{2}/gi).map(byte => parseInt(byte, 16)));
  const view = new DataView(new ArrayBuffer(4));
  view.setUint32(0, slice_index, true); // true express little-endian
  const sliceIndexBlob = new Blob([new Uint8Array(view.buffer)]);
  const fileSliceBlob = file.slice(start, end); 
  let blobToSend = new Blob([hashvalBlob, sliceIndexBlob, fileSliceBlob]);
  data.socket.send(blobToSend)
}

function uploadAll(file) {
  let slice_size = file.req.slice_size;
  let size = file.req.size;
  let i = 0;
  let n = parseInt((size - 1) / slice_size + 1);

  while (i < n) {
    let sendsize = slice_size;
    if (i == n - 1) {
      sendsize = size - i*slice_size;
    }
    uploadSlice(file.f, file.req.file_hash, i, i*slice_size, i*slice_size + sendsize);
    i++;
  }
}

self.onmessage = function(workerMessageIn) {
  let msg = workerMessageIn.data;
  if (typeof msg === 'string') {
    handleCommand(msg);
    return;
  } 

  if (data.socket != null && data.socket.readyState != WebSocket.OPEN) { 
    return;
  }

  let file = {
    f: msg.f,
    req: msg.req,
    slice_idx: msg.slice_idx
  }
  if (file.slice_idx) {
    for (let j = file.slice_idx[0]; j < file.slice_idx[1]; j++) {
      let start = file.req.slice_size*j;
      let end = start + Math.min(file.req.slice_size, file.req.size - start);
      uploadSlice(file.f, file.req.file_hash, j, start, end);
    }
  } else {
    uploadAll(file);
  }
}