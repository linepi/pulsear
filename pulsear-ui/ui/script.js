function toggleTheme() {
  if (data.localConfig.userconfig.theme === 'dark') {
    data.localConfig.userconfig.theme = 'light';
  } else {
    data.localConfig.userconfig.theme = 'dark';
  }
}

function sortTableToggle(column) {
  if (data.localConfig.userconfig.filelist_config["/"].order_by === column) {
    data.localConfig.userconfig.filelist_config["/"].order_asc =
      !data.localConfig.userconfig.filelist_config["/"].order_asc;
  } else {
    data.localConfig.userconfig.filelist_config["/"].order_by = column;
  }
  const tbody = document.querySelector('.file-list tbody');
  let rows = Array.from(tbody.querySelectorAll('tr'));
  sortTableImpl(rows, column, data.localConfig.userconfig.filelist_config['/'].order_asc, (obj) => {
    return obj.querySelector(`td[data-key="${column}"]`).textContent;
  });
  rows.forEach(row => tbody.appendChild(row));
}

function sortTableImpl(rows, sortKey, asc, getContent) {
  let convertSizeToBytes = function (sizeStr) {
    const units = { 'Kb': 1024, 'Mb': 1024 * 1024, 'Gb': 1024 * 1024 * 1024 };
    const regex = /(\d+(?:\.\d+)?)\s*(Kb|Mb|Gb)/;
    const matches = sizeStr.match(regex);
    if (matches) {
      const value = parseFloat(matches[1]);
      const unit = matches[2];
      return value * units[unit];
    }
    return 0; // Return 0 if no matches or in case of invalid format
  }
  rows.sort((a, b) => {
    let valA = getContent(a);
    let valB = getContent(b);

    if (sortKey === 'size') { // Size sorting
      valA = convertSizeToBytes(valA);
      valB = convertSizeToBytes(valB);
    } else if (sortKey === 'create_t' || sortKey === 'modify_t' || sortKey === 'access_t') { // Date sorting
      valA = new Date(valA);
      valB = new Date(valB);
    }

    if (typeof valA === 'string' && typeof valB === 'string') {
      return asc ? valA.localeCompare(valB) : valB.localeCompare(valA);
    } else {
      return asc ? valA - valB : valB - valA;
    }
  });
}

// 
function createFileRowElem(fileElem) {
  let tr = document.createElement('tr');
  let columns = data.localConfig.userconfig.filelist_config['/'].columns;
  columns.forEach(columnName => {
    let td = document.createElement('td'); 
    td.innerHTML = fileElem[columnName]; 
    td.setAttribute('data-key', columnName);
    tr.appendChild(td);
  });

  let td = document.createElement('td');
  let actions = document.createElement('div');
  actions.className = "gg-software-download";
  actions.addEventListener('click', function (evt) {
    downloadFile(fileElem.name);
  });
  td.appendChild(actions);
  tr.appendChild(td);

  return tr;
}

// 
function getFileElem(newFileName) {
  let newFileElem = null;
  fetch(data.api.getfileelem, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json; charset=UTF-8'
    },
    body: JSON.stringify({
      name: newFileName,
      username: data.userCtx.username,
      token: data.userCtx.token
    })
  }).then(response => {
    if (!response.ok) {
      console.error("get file bad response:", response);
      throw new Error(response);
    }
    return response.json();
  }).then(json => {
    newFileElem = json;
  }).catch(e => {
    console.error("get file error ", e);
    return null;
  })
  return newFileElem;
}

function createFileListHeader() {
  let columnName2text = {
    "name": "File Name",
    "size": "Size",
    "create_t": "Create",
    "modify_t": "Modify",
    "access_t": "Access"
  };
  let columns = data.localConfig.userconfig.filelist_config['/'].columns;
  let thead_tr = document.querySelector('thead tr');
  thead_tr.innerHTML = '';
  columns.forEach(columnName => {
    let th = document.createElement('th');
    th.innerHTML = 
      `<th>
        ${columnName2text[columnName]}
        <i class="sort-order-down" v-show="data.localConfig.userconfig.filelist_config['/'].order_by==='${columnName}' &&
          !data.localConfig.userconfig.filelist_config['/'].order_asc"></i>
        <i class=" sort-order-up" v-show="data.localConfig.userconfig.filelist_config['/'].order_by==='${columnName}' &&
          data.localConfig.userconfig.filelist_config['/'].order_asc"></i>
        <div class=" resize-handle"></div>
      </th>`;
    th.addEventListener('click', () => {
      sortTableToggle(columnName);
    });
    thead_tr.appendChild(th);
  });
  let action_th = document.createElement('th');
  action_th.innerHTML = 'Action';
  thead_tr.appendChild(action_th);

  // init file list column resize handle
  const handles = document.querySelectorAll('.resize-handle');
  handles.forEach(handle => {
    let col = handle.parentElement;
    let x = 0;
    let w = 0;
    const mouseDownHandler = function (e) {
      // Get the current mouse position
      x = e.clientX;

      // Calculate the current width of column
      const styles = window.getComputedStyle(col);
      w = parseInt(styles.width, 10);

      // Attach listeners for document's events
      document.addEventListener('mousemove', mouseMoveHandler);
      document.addEventListener('mouseup', mouseUpHandler);
    };

    const mouseMoveHandler = function (e) {
      // Determine how far the mouse has been moved
      const dx = e.clientX - x;

      // Update the width of column
      col.style.width = `${w + dx}px`;
    };

    // When user releases the mouse, remove the existing event listeners
    const mouseUpHandler = function () {
      document.removeEventListener('mousemove', mouseMoveHandler);
      document.removeEventListener('mouseup', mouseUpHandler);
    };

    handle.addEventListener('mousedown', mouseDownHandler);
  })


}

function loadFileList(newFileName) {
  if (!data.userCtx.login) {
    console.log("load file with no login?");
    return;
  }
  console.log("load file");
  createFileListHeader();
  refreshDom();
  // add file list element
  let tbody = document.querySelector('tbody');
  tbody.innerHTML = '';

  fetch(data.api.getfile, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json; charset=UTF-8'
    },
    body: JSON.stringify({
      username: data.userCtx.username,
      token: data.userCtx.token
    })
  }).then(response => {
    if (!response.ok) {
      console.error("get file bad response:", response);
      throw new Error(response);
    }
    return response.json();
  }).then(json => {
    // struct FileListElem {
    //   name: String,
    //   size: String,
    //   create_t: String,
    //   access_t: String,
    //   modify_t: String,
    // }
    sortTableImpl(json.files, data.localConfig.userconfig.filelist_config["/"].order_by,
      data.localConfig.userconfig.filelist_config["/"].order_asc, (file) => {
        let columnVal = file[data.localConfig.userconfig.filelist_config["/"].order_by];
        return columnVal;
      });
    json.files.forEach(fileElem => {
      let tr = createFileRowElem(fileElem);
      tbody.appendChild(tr);

      // if a new file appear in file list, highlight it
      if (fileElem.name === newFileName) {
        tr.childNodes.forEach(td => {
          td.classList.add('highlight-new-file');
          setTimeout(() => {
            td.classList.add('fadeOutAnimation');
          }, 0);
        });
      }
    });
  }).catch(e => {
    console.error("get file error ", e);
  })
}

function uploadFile(evt) {
  console.log('try upload');
  let numberOfActiveWorker = 0;
  data.ws.workers.forEach(worker => {
    if (worker.established) {
      numberOfActiveWorker++;
    }
  });
  if (!data.ws.established || numberOfActiveWorker == 0) {
    notify(false, "please wait, then retry");
    return;
  }
  if (evt.target.files.length == 0) {
    notify(false, "please retry");
    return;
  }
  [...evt.target.files].forEach(function (file) {
    data.uploader.upload(file);
  });
}

// filename: String
function downloadFile(filename) {
  fetch(data.api.getdownloadurl, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json; charset=UTF-8'
    },
    body: JSON.stringify({
      name: filename,
      username: data.userCtx.username,
      token: data.userCtx.token
    })
  }).then(response => {
    if (!response.ok) {
      console.error("get file bad response:", response);
      throw new Error(response);
    }
    response.text().then(text => {
      window.open(data.api.downloadbyurl + `/${data.userCtx.username}/${text}`, '_blank');
    });
  }).catch(e => {
    console.error(e);
  })
}

function onDragOver(evt) {
  evt.preventDefault();
}

function onDrop(evt) {
  console.log("File(s) dropped");
  evt.preventDefault();

  if (evt.dataTransfer.items) {
    [...evt.dataTransfer.items].forEach((item, i) => {
      if (item.kind === "file") {
        const file = item.getAsFile();
        data.uploader.upload(file);
      }
    });
  } else {
    [...evt.dataTransfer.files].forEach((file, i) => {
      data.uploader.upload(file);
    });
  }
}
