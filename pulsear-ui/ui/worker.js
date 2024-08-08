/**
  file: { 
    f: file, 
    req: request, 
    tr: null,
    name_td: null,
    name_overlay: null
  };
*/

self.onmessage = function(msg) {
  let file = msg[0];
  let socket = msg[1];

  function uploadSlice(file, hashval, i, start, end) {
    const hashvalBlob = new Uint8Array(hashval.match(/[\da-f]{2}/gi).map(byte => parseInt(byte, 16)));
    const view = new DataView(new ArrayBuffer(4));
    view.setUint32(0, i, true); // true express little-endian
    const sliceIndexBlob = new Blob([new Uint8Array(view.buffer)]);
    const fileSliceBlob = file.slice(start, end); 
    let blobToSend = new Blob([hashvalBlob, sliceIndexBlob, fileSliceBlob]);
    socket.send(blobToSend)
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

  uploadAll(file);
}