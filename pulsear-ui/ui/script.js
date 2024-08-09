function toggleTheme() {
  if (data.localConfig.theme === 'dark') {
    data.localConfig.theme = 'light';
  } else {
    data.localConfig.theme = 'dark';
  }
}

function sortTableToggle(column) {
  if (data.localConfig.fileSort.column === column) {
    data.localConfig.fileSort.order = data.localConfig.fileSort.order === 'asc' ? 'desc' : 'asc';
  } else {
    data.localConfig.fileSort.column = column;
    data.localConfig.fileSort.order = 'asc';
  }
  const tbody = document.querySelector('.file-list tbody');
  let rows = Array.from(tbody.querySelectorAll('tr'));
  sortTableImpl(rows, column, data.localConfig.fileSort.order, (obj) => {
    return obj.querySelector(`td[data-key="${column}"]`).textContent;
  });
  rows.forEach(row => tbody.appendChild(row));
}

function sortTableImpl(rows, sortKey, order, getContent) {
  let isAscending = order === 'asc' ? true : false;
  let convertSizeToBytes = function(sizeStr) {
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
    } else if (sortKey === 'create' || sortKey === 'modify' || sortKey === 'access') { // Date sorting
      valA = new Date(valA);
      valB = new Date(valB);
    }

    if (typeof valA === 'number' && typeof valB === 'number') {
      return isAscending ? valA - valB : valB - valA;
    } else {
      return isAscending ? valA.localeCompare(valB) : valB.localeCompare(valA);
    }
  });
}

// 
function createFileRowElem(fileElem) {
  let tr = document.createElement('tr');
  let td1 = document.createElement('td'); td1.innerHTML = fileElem.name; td1.setAttribute('data-key', 'name');
  let td2 = document.createElement('td'); td2.innerHTML = fileElem.size; td2.setAttribute('data-key', 'size');
  let td3 = document.createElement('td'); td3.innerHTML = fileElem.create_t; td3.setAttribute('data-key', 'create_t');
  let td4 = document.createElement('td'); td4.innerHTML = fileElem.modify_t; td4.setAttribute('data-key', 'modify_t');
  let td5 = document.createElement('td'); td5.innerHTML = fileElem.access_t; td5.setAttribute('data-key', 'access_t');

  let td6 = document.createElement('td'); 
  let actions = document.createElement('div'); 
  actions.className = "gg-software-download";
  actions.addEventListener('click', function(evt) {
    downloadFile(fileElem.name);
  });

  td6.appendChild(actions);
  tr.appendChild(td1);
  tr.appendChild(td2);
  tr.appendChild(td3);
  tr.appendChild(td4);
  tr.appendChild(td5);
  tr.appendChild(td6);
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

function loadFileList(newFileName) {
  if (!data.userCtx.login) {
    console.log("load file with no login?");
    return;
  }
  console.log("load file");
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
    sortTableImpl(json.files, data.localConfig.fileSort.column,
      data.localConfig.fileSort.order, (file) => {
        switch (data.localConfig.fileSort.column) {
        case "name": return file.name;
        case "size": return file.size;
        case "create_t": return file.create_t;
        case "modify_t": return file.modify_t;
        case "access_t": return file.access_t;
        }
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
  if (evt.target.files.length == 0) {
    notify(false, "please retry");
    return;
  }
  [...evt.target.files].forEach(function(file) {
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

function onResponseCode(code) {
  // if server responsed code is enum::Ok, the code is "Ok" which is a string
  // else enum::Err, then the code is { "Err": errmsg } which is an object
  if (typeof code === "object") {
    throw new Error(code.Err);
  }
}
