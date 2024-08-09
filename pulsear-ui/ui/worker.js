// global value for this worker
let data = {
  id: null,
  builded: null,
  wsUri: null,
  resources_prefix: null,
  socket: null,
};

function start(id, wsUri, resources_prefix) {
  data.id = id;
  data.wsUri = wsUri;
  data.resources_prefix = resources_prefix;

  importScripts(`${resources_prefix}ws.js`, `${resources_prefix}uploader.js`);

  let socket = new WebSocket(wsUri);

  socket.onopen = evt => {
    console.log(`worker ${data.id} socket connected`);
    let msg = new WsMessage(
      WsSender.withUser("", ""),
      WsMessageClass.withCreateWsWorker(id),
      WsDispatchType.Server
    );
    socket.send(msg.toJson());
  }

  socket.onmessage = evt => {
    let ws_message = WsMessage.fromJson(evt.data);
    console.log(`worker ${data.id} socket receive ${evt.data}`);
    if (ws_message.msg.is(WsMessageClass.CreateWsWorker)) {
      let id_from_server = ws_message.msg.content;
      if (data.id != id_from_server) {
        console.error("internal error!");
      }
      data.builded = true;
      postMessage('builded');
    }
  }

  socket.onclose = evt => {
    console.log(`worker ${data.id} socket disconnected`);
    socket = null;
    data.builded = false;
    postMessage('disconnect');
  }

  socket.onerror = evt => {
    console.log(`worker ${data.id} socket error: `, evt);
    socket = null;
    data.builded = false;
    postMessage('disconnect');
  }

  data.socket = socket;
}

function handleCommand(cmd) {
  let args = cmd.split(' ');
  if (args[0] === 'start') {
    start(parseInt(args[1]), args[2], args[3]);
    return;
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

function str(obj) {
  return JSON.stringify(obj, null, "  ");
}

self.onmessage = function(workerMessageIn) {
  let msg = workerMessageIn.data;
  if (typeof msg === 'string') {
    handleCommand(msg);
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