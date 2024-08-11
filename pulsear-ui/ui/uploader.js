function getRandomInt(ceil) {
  let bigNum = Math.floor(Math.random() * 10000000);
  return bigNum % ceil;
}

function pseudoSha256(input) {
  let seed = 0x12345678;
  for (let i = 0; i < input.length; i++) {
    seed = (seed * 31 + input.charCodeAt(i)) & 0xFFFFFFFF;
  }
  let pseudoHash = '';
  for (let i = 0; i < 32; i++) {
    seed = (seed * 31 + i) & 0xFFFFFFFF;
    pseudoHash += ('00' + (seed >>> 0).toString(16)).slice(-2);
  }
  return pseudoHash;
}

class Uploader {
  // #websocketNum
  // #workerNum

  // constructor(websocketNum, workerNum) {
  //   this.#websocketNum = websocketNum;
  //   this.#workerNum = workerNum;
  // }
  #files
  #sliceSize = 40960

  constructor() {
    this.#files = {};
  }

  chooseWorker() {
    this.workerNum = data.localConfig.userconfig.web_worker_num;
    let newChoose;

    do {
      if (this.lastChoose == null) {
        newChoose = getRandomInt(this.workerNum);
      } else {
        do {
          newChoose = getRandomInt(this.workerNum);
        } while (newChoose == this.lastChoose);
      }
    } while (!data.ws.workers[newChoose].established);

    this.lastChoose = newChoose;
    return newChoose;
  }

  onWsMessage(ws_message) {
    if (ws_message.msg.is(WsMessageClass.FileSendable)) {
      let file_sendable_resp = ws_message.msg.content;
      let hashval = file_sendable_resp.hashval;
      let user_ctx_hash = file_sendable_resp.user_ctx_hash;
      // other client of this user should create a new one
      if (!this.#files[hashval]) {
        this.#files[hashval] = {};
      }
      let file = this.#files[hashval];
      if (data.userCtx.user_ctx_hash === user_ctx_hash) {
        file.isUploader = true;
        console.log("IM UPLOADER");
      } else {
        file.isUploader = false;
        console.log("IM NOT UPLOADER");
      }
      if (file_sendable_resp.file_elem != null) {
        let file_elem = file_sendable_resp.file_elem;
        let req = file_sendable_resp.req;

        let tbody = document.querySelector('tbody');
        let tr = createFileRowElem(file_elem);
        let name_td = tr.childNodes[0];
        let overlay = document.createElement('div');
        overlay.className = 'td-overlay';
        overlay.style.opacity = 0.85;
        name_td.style.position = 'relative';
        name_td.appendChild(overlay);
        tbody.appendChild(tr);

        file.tr = tr;
        file.name_td = name_td;
        file.name_overlay = overlay;
        file.req = req;
        file.upload = {
          nr_slice_all: parseInt((req.size - 1) / req.slice_size + 1),
          nr_slice_ok: 0
        };
        if (file.isUploader) {
          let wholeworker = 1;
          let wholeslice = parseInt((file.req.size - 1) / file.req.slice_size + 1);
          let worker_slice_devide = parseInt((wholeslice - 1) / wholeworker + 1);
          let worker_slice_n = 0;
          if (worker_slice_devide == 0) {
            worker_slice_devide++;
          }
          for (let i = 0; i < wholeworker && worker_slice_n < wholeslice; i++) {
            let slice_start = i * worker_slice_devide;
            let slice_send = Math.min(worker_slice_devide, wholeslice - worker_slice_n);
            giveWorkerMsg(file.worker_id = this.chooseWorker(), {
              req: file.req,
              f: file.f,
              slice_idx: [slice_start, slice_start + slice_send],
            });
            worker_slice_n += slice_send;
          }
        }

        this.focusRow(file.tr);
        this.updateUploadStatus(file.name_overlay, file.upload, false);
        this.notifyWrapper(false, "upload file " + file_elem.name, file.isUploader);
      } else {
        this.notifyWrapper(false, "sorry, you cannot send ", file.req.name,
          file.isUploader);
        if (file.isUploader) {
          delete (this.#files[file_sendable_resp.hashval]);
        }
      }
    } else if (ws_message.msg.is(WsMessageClass.FileResponse)) {
      let resp = ws_message.msg.content;
      const file = this.#files[resp.file_hash];
      if (file == null || file.upload == null) {
        console.log('file has been deleted: ', file);
      }
      if (resp.status === "Ok") {
        file.upload.nr_slice_ok += resp.slice_idx[1] - resp.slice_idx[0];
        // NOTE: the bitset range is inclusive
        this.updateUploadStatus(file.name_overlay, file.upload);
        // when all is zero, bs become empty
        if (this.#files[resp.file_hash].isUploader) {
          file.bs.setRange(resp.slice_idx[0], resp.slice_idx[1] - 1, 0);
          file.bs_updated = true;
          if (file.bs.isEmpty()) {
            // tell server this is finished
            let msg = new WsMessage(
              WsSender.withUser(data.userCtx.username, data.userCtx.user_ctx_hash),
              WsMessageClass.withFileResponse({
                name: file.req.name,
                file_hash: resp.file_hash,
                slice_idx: [0, parseInt((file.req.size - 1) / this.#sliceSize)],
                status: "Finish"
              }),
              WsDispatchType.Server
            );
            wssend(msg.toJson());
          }
        }
      } else if (resp.status === "Resend" && this.#files[resp.file_hash].isUploader) {
        giveWorkerMsg(file.worker_id, {
          req: file.req,
          f: file.f,
          slice_idx: [resp.slice_idx[0], resp.slice_idx[1]]
        });
      } else if (resp.status === "Fatalerr") {
        this.notifyWrapper(true, `upload ${resp.name} error`, this.#files[resp.file_hash].isUploader);
        delete (this.#files[resp.file_hash]);
      } else if (resp.status === "Finish") {
        // server response finish after receive client's finish
        this.updateUploadStatus(file.name_overlay, file.upload, true);
        this.onFileUploaded(file);
        delete (this.#files[resp.file_hash]);
      }
    } else if (ws_message.msg.is(WsMessageClass.PleaseSend)) {
      let file_hash = ws_message.msg.content;
      let file = this.#files[file_hash];
      if (file.isUploader) {
        if (file.bs_updated) this.bsSendAllLeft(file);
        else this.bsSendOne(file);
        file.bs_updated = false;
      }
    }
  }

  bsSendAllLeft(file) {
    let ones = file.bs.toArray();
    let lastNotSendIdx = ones[0]; // [1,2,3,5,6,7,11,12]
    for (let i = 1; i < ones.length; i++) {
      if (ones[i] - ones[i - 1] > 1) {
        giveWorkerMsg(file.worker_id, {
          req: file.req,
          f: file.f,
          slice_idx: [lastNotSendIdx, ones[i - 1] + 1]
        });
        lastNotSendIdx = ones[i];
      }
    }
    giveWorkerMsg(file.worker_id, {
      req: file.req,
      f: file.f,
      slice_idx: [lastNotSendIdx, ones[ones.length - 1] + 1]
    });
  }

  bsSendOne(file) {
    let leastNotSend = file.bs.lsb();
    giveWorkerMsg(file.worker_id, {
      req: file.req,
      f: file.f,
      slice_idx: [leastNotSend, leastNotSend + 1]
    });
  }

  focusRow(tr) {
    let now = new Date();
    if (!this.focusMilli || now.getMilliseconds() - this.lastFocusMilli.getMilliseconds() > 1000) {
      tr.scrollIntoView({ behavior: 'smooth', block: 'center' });
      this.lastFocusMilli = new Date();
    } else {
      console.log(`not eager to scroll, lastTime ${this.lastFocusMilli}, now ${now}`);
    }
  }

  // status {
  //   nr_slice_all: , 
  //   nr_slice_ok: ,
  // }
  updateUploadStatus(overlay, status, ok) {
    let percent = status.nr_slice_ok / status.nr_slice_all; // 这是一个0到1之间的值
    let upload_percent = percent * 100;
    if (upload_percent > 100) upload_percent = 100;
    if (ok) {
      overlay.style.opacity = 0;
    } else if (percent != 1) {
      overlay.style.opacity = (0.85 - 0.5 * percent).toFixed(2); // 从20%的透明度开始到100%的不透明
    }
    overlay.textContent = `${upload_percent.toFixed(2)}% uploaded`;
  }


  onFileUploaded(file) {
    let suffix = "";
    if (!file.isUploader) {
      suffix += " in other place"
    }

    let filename = file.req.name;
    file.tr.childNodes.forEach(td => {
      td.classList.add('highlight-new-file');
      setTimeout(() => {
        td.classList.add('fadeOutAnimation');
      }, 0);
    });
    notify(false, `upload ${filename} success ${suffix}`);
  }

  notifyWrapper(important, msg, isUploader) {
    notify(important, `${!isUploader ? "In other place: " : ""}${msg}`)
  }

  // a file hash stand for this transfer
  hash(file) {
    let combinedStr = file.name + data.userCtx.username +
      file.size.toString() + file.lastModified.toString() + new Date().toString();
    return pseudoSha256(combinedStr);
  }

  upload(file) {
    console.log('upload ', file);
    let hashval = this.hash(file);
    let request = {
      username: data.userCtx.username,
      name: file.name,
      size: file.size,
      slice_size: this.#sliceSize,
      last_modified_t: file.lastModified,
      file_hash: hashval
    };
    let msg = new WsMessage(
      WsSender.withUser(data.userCtx.username, data.userCtx.user_ctx_hash),
      WsMessageClass.withFileRequest(request),
      WsDispatchType.Server
    );
    wssend(msg.toJson());

    /**
     * There, use bitset to have full file sending information, even in network instability situation.
     * Only client has this information. So server do not know if the file has all beed sended.
     */
    let slice_num = parseInt((file.size - 1) / this.#sliceSize + 1);
    let bs = new BitSet();
    // 1 stand for not send
    bs.setRange(0, slice_num - 1, 1);
    this.#files[hashval] = {
      // uploader
      f: file,
      req: request,
      worker_id: -1,
      bs: bs,
      bs_updated: false,
      // uploader

      upload: null,
      isUploader: true,
      tr: null,
      name_td: null,
      name_overlay: null,
    };
  }
}