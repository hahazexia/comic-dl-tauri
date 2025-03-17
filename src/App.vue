<script setup lang="ts">
import { ref, reactive, onMounted, computed } from "vue";
import { listen } from '@tauri-apps/api/event';
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from '@tauri-apps/api/window';
import { toast } from 'vue3-toastify';
import 'vue3-toastify/dist/index.css';

interface Tasks {
  id: number,
  dl_type: string,
  status: string,
  local_path: string,
  url: string,
  author: string,
  comic_name: string,
  progress: string,
  count: number,
  now_count: number,
  error_vec: string,
  done: boolean,
};

interface DownloadEvent {
  id: number,
  progress: string,
  count: number,
  now_count: number,
  error_vec: string,
  status: string,
}

const dl_type_map = ref<any>({
  juan: '单行本',
  hua: '单话',
  fanwai: '番外篇',
  current: '',
});
const tasks_current = reactive<Tasks[]>([]);
const tasks_all = reactive<Tasks[]>([]);
const active_menu = ref('all');
const del_task = ref<Tasks>({
  id: 0,
  dl_type: "",
  status: "",
  local_path: "",
  url: "",
  author: "",
  comic_name: "",
  progress: "",
  count: 0,
  now_count: 0,
  error_vec: "",
  done: false
});
const deleteOneOpen = ref(false);
const deleteAllOpen = ref(false);
const errorInfoOpen = ref(false);
const error_info = ref('');

const isMenuVisible = ref(false);
const menuX = ref(0);
const menuY = ref(0);
const currentMenuData = ref<Tasks>();

const task_downloading = computed(() => {
  return tasks_all.filter(task => task.status === 'downloading');
});
const task_finished = computed(() => {
  return tasks_all.filter(task => task.status === 'finished');
});
const task_stopped = computed(() => {
  return tasks_all.filter(task => task.status === 'stopped');
});
const task_failed = computed(() => {
  return tasks_all.filter(task => task.status === 'failed');
});
const task_waiting = computed(() => {
  return tasks_all.filter(task => task.status === 'waiting');
});

async function add() {
  await invoke('add').then(console.log).catch(console.error);
}

async function setting() {
  await invoke('setting');
}

async function folder() {
  await invoke('open_cache_folder');
}

async function about() {
  await invoke('open_about_winfow');
}

function switchMenu(menu: string) {
  if (active_menu.value === menu) return;
  active_menu.value = menu;
  calc_tasks_current(menu);
}

function calc_tasks_current(menu?: any) {
  tasks_current.splice(0);
  let temp_menu = menu || active_menu.value;
  switch (temp_menu) {
    case 'all':
      tasks_current.push(...tasks_all);
      break;
    case 'downloading':
      tasks_current.push(...task_downloading.value);
      break;
    case 'finished':
      tasks_current.push(...task_finished.value);
      break;
    case 'stopped':
      tasks_current.push(...task_stopped.value);
      break;
    case 'failed':
      tasks_current.push(...task_failed.value);
      break;
    case 'waiting':
      tasks_current.push(...task_waiting.value);
      break;
  }
  sortTasks();
}

function deleteOneTask(data: any) {
  del_task.value = data;
  openDeleteOneModal();
}

function sortTasks() {
  const statusOrder = ['downloading', 'waiting', 'stopped', 'failed', 'finished'];

  tasks_all.sort((a, b) => {
    const indexA = statusOrder.indexOf(a.status);
    const indexB = statusOrder.indexOf(b.status);

    // 先比较 status 在 statusOrder 中的索引
    if (indexA !== indexB) {
      return indexA - indexB;
    }

    // 如果 status 相同，比较 author
    // if (a.author < b.author) {
    //   return -1;
    // } else if (a.author > b.author) {
    //   return 1;
    // }

    // 如果 author 也相同，比较 now_count
    // if (b.now_count !== a.now_count) {
    //   return b.now_count - a.now_count;
    // }

    // 如果 now_count 相同，比较 count
    return a.count - b.count;
  });
  tasks_current.sort((a, b) => {
    const indexA = statusOrder.indexOf(a.status);
    const indexB = statusOrder.indexOf(b.status);

    // 先比较 status 在 statusOrder 中的索引
    if (indexA !== indexB) {
      return indexA - indexB;
    }

    // 如果 status 相同，比较 author
    // if (a.author < b.author) {
    //   return -1;
    // } else if (a.author > b.author) {
    //   return 1;
    // }

    // 如果 author 也相同，比较 now_count
    // if (b.now_count !== a.now_count) {
    //   return b.now_count - a.now_count;
    // }

    // 如果 now_count 相同，比较 count
    return a.count - b.count;
  });
}

function showErrorInfo(data: any) {
  error_info.value = data.error_vec;
  errorInfoOpen.value = true;
}
function closeErrorInfoModal() {
  errorInfoOpen.value = false;
}

const openDeleteOneModal = () => {
  deleteOneOpen.value = true;
};

const closeDeleteOneModal = () => {
  deleteOneOpen.value = false;
};
const closeDeleteAllModal = () => {
  deleteAllOpen.value = false;
};
const confirmDeleteAll = async () => {
  let res: any = await invoke('delete_all');
  tasks_current.splice(0);
  tasks_all.splice(0);
  tasks_all.push(...res);
  tasks_current.push(...res);

  sortTasks();

  deleteAllOpen.value = false;
};

const startOrPause = async (data: any, status: string) => {
  console.log(data);
  startTask(data, status);
};

async function startTask(data: any, status: string) {
  await invoke("start_or_pause", { id: data.id, status: status });
}

const confirmDeleteOne = async () => {
  let del_id = await invoke("delete_tasks", { id: del_task.value?.id });
  const index = tasks_all.findIndex(item => item.id === del_id);
  const index2 = tasks_current.findIndex(item => item.id === del_id);
  if (index !== -1) {
    tasks_all.splice(index, 1);
    tasks_current.splice(index2, 1);
  }

  deleteOneOpen.value = false;
};

const startAll = async () => {
  let res: any = await invoke('start_all');
  console.log(res, 'start all res');
  tasks_current.splice(0);
  tasks_all.splice(0);
  tasks_all.push(...res.tasks);
  tasks_current.push(...res.tasks);

  sortTasks();

  for (let i of res.changed) {
    if (i.status === 'downloading') {
      startTask({ id: i.id }, 'downloading');
    }
  }
};

const pauseAll = async () => {
  let res: any = await invoke('pause_all');
  tasks_current.splice(0);
  tasks_all.splice(0);
  tasks_all.push(...res);
  tasks_current.push(...res);

  sortTasks();
};
const pauseAllWaiting = async () => {
  let res: any = await invoke('pause_all_waiting');

  tasks_current.splice(0);
  tasks_all.splice(0);
  tasks_all.push(...res);
  tasks_current.push(...res);

  sortTasks();
};
const deleteAll = async () => {
  deleteAllOpen.value = true;
};

function showContextMenu(e: any, data: any) {
  menuX.value = e.pageX;
  menuY.value = e.pageY;
  isMenuVisible.value = true;
  currentMenuData.value = data;
};

async function handleMenuItemClick() {
  let res: any = await invoke('get_setting');
  let map: any = {
    juan: '单行本',
    hua: '单话',
    fanwai: '番外篇',
    current: '',
  };
  let name = `${currentMenuData.value?.comic_name}_${map[currentMenuData.value?.dl_type as any]}`;
  let download_dir = `${res.download_dir}${currentMenuData.value?.author ? (currentMenuData.value?.author + '/') : ''}${name}`;
  await invoke('open_dir', {
    dir: download_dir,
  });
  isMenuVisible.value = false;
}

function blankClick() {
  isMenuVisible.value = false;
}

onMounted(() => {
  listen('err_msg_main', (e: any) => {
    toast(`${e.payload}`, {
      position: toast.POSITION.TOP_CENTER,
      type: 'error',
      autoClose: 2000,
    });
  });

  listen('info_msg_main', (e: any) => {
    toast(`${e.payload}`, {
      position: toast.POSITION.TOP_CENTER,
      type: 'info',
      autoClose: 2000,
    });
  });

  listen('progress', (e: any) => {
    let message: DownloadEvent = e.payload;
    const index = tasks_all.findIndex(item => item.id === message.id);
    const index2 = tasks_current.findIndex(item => item.id === message.id);

    tasks_all[index].progress = message.progress;
    tasks_all[index].now_count = message.now_count;
    tasks_all[index].error_vec = message.error_vec;
    tasks_all[index].status = message.status;


    tasks_current[index2].progress = message.progress;
    tasks_current[index2].now_count = message.now_count;
    tasks_current[index2].error_vec = message.error_vec;
    tasks_current[index2].status = message.status;

    sortTasks();
  });

  listen('new_task', (e: any) => {
    tasks_all.push(e.payload);
    calc_tasks_current();
  });

  listen('task_status', (e: any) => {
    let data = e.payload;
    tasks_all.forEach((i: any) => {
      if (i.id === Number(data.id)) {
        i.status = data.status;
      }
    });
    tasks_current.forEach((i: any) => {
      if (i.id === Number(data.id)) {
        i.status = data.status;
      }
    });

    sortTasks();
  });

  listen('start_waiting', (e: any) => {
    let will_starts: any = e.payload;
    will_starts.forEach((i: any) => {
      startTask({ id: i }, 'downloading');
    });
  });

  const appWindow = getCurrentWindow();
  document.querySelector('.menu')?.addEventListener('mousedown', (e: any) => {
    if (e.buttons === 1) {
      e.detail === 2
        ? appWindow.toggleMaximize() // Maximize on double click
        : appWindow.startDragging(); // Else start dragging
    }
  });

  (async () => {
    let res: any = await invoke('get_tasks');
    console.log(res, 'get_tasks');
    console.log(JSON.stringify(res), 'get_tasks');
    tasks_all.push(...res);
    tasks_current.push(...res);

    sortTasks();
  })();
});
</script>

<template>
  <div class="main" @click="blankClick">
    <div class="menu">
      <div class="menu-option all" :class="{ active: active_menu === 'all' }" @click="() => switchMenu('all')">All
        Tasks<span class="num" v-text="tasks_all.length"></span></div>
      <div class="menu-option downloading" :class="{ active: active_menu === 'downloading' }"
        @click="() => switchMenu('downloading')">Downloading<span class="num" v-text="task_downloading.length"></span>
      </div>
      <div class="menu-option waiting" :class="{ active: active_menu === 'waiting' }"
        @click="() => switchMenu('waiting')">Waiting<span class="num" v-text="task_waiting.length"></span></div>
      <div class="menu-option stopped" :class="{ active: active_menu === 'stopped' }"
        @click="() => switchMenu('stopped')">Stopped<span class="num" v-text="task_stopped.length"></span></div>
      <div class="menu-option finished" :class="{ active: active_menu === 'finished' }"
        @click="() => switchMenu('finished')">Finished<span class="num" v-text="task_finished.length"></span></div>
      <div class="menu-option failed" :class="{ active: active_menu === 'failed' }" @click="() =>
        switchMenu('failed')">Failed<span class="num" v-text="task_failed.length"></span></div>
      <div class="add" title="create new task" @click="add"></div>
      <div class="about" title="About" @click="about"></div>
      <div class="folder" title="open cache folder" @click="folder"></div>
      <div class="setting" title="setting" @click="setting"></div>
    </div>
    <div class="list">
      <div class="list-tool">
        <div class="list-tool-btn start-all" title="start all" @click="startAll"></div>
        <div class="list-tool-btn pause-all" title="pause all" @click="pauseAll"></div>
        <div class="list-tool-btn pause-all-waiting" title="pause all waiting" @click="pauseAllWaiting"></div>
        <div class="list-tool-btn delete-all" title="delete all not downloading" @click="deleteAll"></div>
      </div>
      <div class="list-item" v-for="data in tasks_current" :key="data.id"
        @contextmenu.prevent.capture="(e) => showContextMenu(e, data)" :style="{ '--progress': `${data.progress}%` }">
        <div class="name" :class="{ 'downloading-name': data.status === 'downloading' }"
          v-text="`${data.comic_name}_${dl_type_map[data.dl_type]}`"
          :title="`${data.comic_name}_${dl_type_map[data.dl_type]}`"></div>
        <div class="desc">
          <div class="status" :class="data.status" v-text="data.status"></div>
          <div class="progress-num" v-text="`${data.progress}%`"></div>
          <div class="progress-count" v-text="`${data.now_count}/${data.count}`"></div>

          <div class="info" v-text="`id:${data.id}`"></div>
          <div class="info" v-text="`author:${data.author}`"></div>
          <div class="info" v-text="`dl_type:${data.dl_type}`"></div>
        </div>
        <div class="progress">
          <div class="progress-inner" :style="{
            width: `${data.progress}%`,
          }"></div>
        </div>
        <div class="tool-bar">
          <div class="left"></div>
          <div class="right">
            <div v-if="data.status !== 'finished'" :class="{
              pause: data.status === 'downloading' || data.status === 'waiting',
              start: data.status === 'stopped' || data.status === 'failed',
            }"
              @click="() => startOrPause(data, (data.status === 'downloading' || data.status === 'waiting') ? 'stopped' : 'downloading')">
            </div>
            <div class="delete" @click="() => deleteOneTask(data)"></div>
            <div v-if="data.error_vec.length > 2" class="error-info" @click="() => showErrorInfo(data)"></div>
          </div>
        </div>
      </div>
      <div class="no-task" v-if="!tasks_current || tasks_current?.length === 0">No task found</div>
    </div>
    <dialog class="delete-dialog" :open="deleteOneOpen" @close="deleteOneOpen = false">
      <div class="delete-title" v-text="`Delete this task?`"></div>
      <p class="delete-item" v-text="`${del_task?.comic_name}_${dl_type_map?.[del_task?.dl_type]}`"></p>
      <div class="delete-operation">
        <button class="delete-btn" @click="confirmDeleteOne">confirm</button>
        <button class="delete-btn" @click="closeDeleteOneModal">cancel</button>
      </div>
    </dialog>
    <dialog class="delete-dialog" :open="deleteAllOpen" @close="deleteAllOpen = false">
      <div class="delete-title" v-text="`Delete all not downloading task?`"></div>
      <div class="delete-operation">
        <button class="delete-btn" @click="confirmDeleteAll">confirm</button>
        <button class="delete-btn" @click="closeDeleteAllModal">cancel</button>
      </div>
    </dialog>
    <dialog class="error-info-dialog" :open="errorInfoOpen" @close="errorInfoOpen = false">
      <div class="info" v-text="error_info"></div>
      <div class="delete-operation">
        <button class="delete-btn" @click="closeErrorInfoModal">cancel</button>
      </div>
    </dialog>
    <div v-if="isMenuVisible" class="custom-menu" :style="{ left: menuX + 'px', top: menuY + 'px' }">
      <ul>
        <li @click="handleMenuItemClick">open download dir</li>
      </ul>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.main {
  display: flex;
  width: 100%;
  height: 100%;

  .menu {
    position: relative;
    width: 200px;
    height: 100%;
    background-color: #fff;
    border-right: 1px solid #CECECE;

    .add {
      cursor: pointer;
      background-image: url('./img/add.svg');
      background-repeat: no-repeat;
      background-size: contain;
      width: 16px;
      height: 16px;
      position: absolute;
      left: 10px;
      bottom: 10px;

      &:hover {
        filter: hue-rotate(180deg) brightness(0.8) saturate(2);
      }
    }

    .about {
      cursor: pointer;
      background-image: url('./img/info2.svg');
      background-repeat: no-repeat;
      background-size: contain;
      width: 20px;
      height: 18px;
      position: absolute;
      right: 70px;
      bottom: 10px;

      &:hover {
        filter: hue-rotate(180deg) brightness(0.8) saturate(2);
      }
    }

    .folder {
      cursor: pointer;
      background-image: url('./img/folder.svg');
      background-repeat: no-repeat;
      background-size: contain;
      width: 20px;
      height: 20px;
      position: absolute;
      right: 40px;
      bottom: 10px;

      &:hover {
        filter: hue-rotate(180deg) brightness(0.8) saturate(2);
      }
    }

    .setting {
      cursor: pointer;
      background-image: url('./img/setting.svg');
      background-repeat: no-repeat;
      background-size: contain;
      width: 18px;
      height: 18px;
      position: absolute;
      right: 10px;
      bottom: 10px;

      &:hover {
        filter: hue-rotate(180deg) brightness(0.8) saturate(2);
      }
    }

    .menu-option {
      display: flex;
      justify-content: space-between;
      align-items: center;
      position: relative;
      cursor: pointer;
      font-size: 12px;
      color: #212121;
      line-height: 50px;
      padding-left: 40px;
      padding-right: 10px;

      &::before {
        content: '';
        display: block;
        width: 20px;
        height: 20px;
        background-repeat: no-repeat;
        background-size: contain;
        position: absolute;
        left: 10px;
        top: 50%;
        transform: translateY(-50%);
      }

      &.all::before {
        background-image: url('./img/all.svg');
      }

      &.downloading::before {
        background-image: url('./img/download.svg');
      }

      &.waiting::before {
        background-image: url('./img/wait.svg');
      }

      &.finished::before {
        background-image: url('./img/ok.svg');
      }

      &.stopped::before {
        background-image: url('./img/stop.svg');
      }

      &.failed::before {
        background-image: url('./img/failed.svg');
      }

      &.active {
        background-color: #F5F5F5;
      }

      &:hover {
        background-color: #F5F5F5;
        filter: hue-rotate(180deg) brightness(0.8) saturate(2);
      }

      .num {
        font-size: 12px;
        color: #5D5D5D;
      }
    }
  }

  .list {
    .no-task {
      padding: 10px;
      font-size: 12px;
    }

    .list-tool {
      position: fixed;
      width: 100%;
      top: 0;
      left: 201px;
      padding: 0px 10px;
      display: flex;
      background-color: #fff;
      z-index: 2;
      border-bottom: 1px solid #CECECE;

      .list-tool-btn {
        cursor: pointer;
        margin-left: 10px;
        font-size: 12px;
        width: 20px;
        height: 20px;
        background-repeat: no-repeat;
        background-size: contain;

        &:hover {
          filter: hue-rotate(180deg) brightness(0.8) saturate(2);
        }

        &.start-all {
          background-image: url('./img/start.svg');
        }

        &.pause-all {
          background-image: url('./img/pause.svg');
        }

        &.pause-all-waiting {
          background-image: url('./img/pause2.svg');
        }

        &.delete-all {
          background-image: url('./img/delete.svg');
        }
      }
    }

    padding-top: 40px;
    max-height: 100%;
    overflow-y: auto;
    flex: 1;

    .list-item {
      cursor: pointer;
      position: relative;
      padding: 20px 0px;

      &:hover {
        & ::before {
          filter: hue-rotate(180deg) brightness(0.8) saturate(2);
        }
      }

      ::before {
        content: "";
        position: absolute;
        top: 0;
        left: 0;
        width: var(--progress);
        height: 100%;
        background-color: rgba(144, 176, 215, 0.022);
        z-index: -1;
      }

      .name {
        font-size: 12px;
        font-weight: bold;
        display: -webkit-box;
        -webkit-box-orient: vertical;
        -webkit-line-clamp: 2;
        overflow: hidden;
        text-overflow: ellipsis;
        word-break: break-all;
      }

      .downloading-name {
        color: #4872ac;
      }

      .desc {
        margin-top: 5px;
        display: flex;
        justify-content: flex-start;

        .status {
          padding-left: 20px;
          position: relative;

          &::before {
            content: '';
            display: block;
            width: 12px;
            height: 12px;
            background-repeat: no-repeat;
            background-size: contain;
            position: absolute;
            left: 0;
            top: 50%;
            transform: translateY(-50%);
          }
        }

        .downloading::before {
          background-image: url('./img/download.svg');
        }

        .waiting::before {
          background-image: url('./img/wait.svg');
        }

        .finished::before {
          background-image: url('./img/ok.svg');
        }

        .stopped::before {
          background-image: url('./img/stop.svg');
        }

        .failed::before {
          background-image: url('./img/failed.svg');
        }

        .status,
        .progress-num,
        .progress-count,
        .info {
          font-size: 12px;
        }

        .progress-num {
          margin-left: 5px;
          color: #4872ac;
        }

        .progress-count {
          margin-left: 10px;
        }

        .info {
          margin-left: 30px;
        }
      }

      .progress {
        width: 100%;
        height: 3px;
        background-color: #E6E6E6;
        margin: 5px 0;

        .progress-inner {
          height: 3px;
          max-width: 100%;
          background-color: #4872ac;
          transition: width 2s ease-in-out;
        }
      }

      .tool-bar {
        display: flex;
        justify-content: space-between;
        align-content: center;
        padding-right: 5%;

        .right {
          display: flex;
          justify-content: flex-end;
          align-content: center;

          .pause,
          .start,
          .delete,
          .error-info {
            background-repeat: no-repeat;
            background-size: contain;
            width: 20px;
            height: 20px;
            cursor: pointer;

            &:hover {
              background-color: #F5F5F5;
              filter: hue-rotate(180deg) brightness(0.8) saturate(2);
            }
          }

          .error-info {
            background-image: url('./img/info.svg');
          }

          .pause {
            background-image: url('./img/pause.svg');
          }

          .start {
            background-image: url('./img/start.svg');
          }

          .delete {
            background-image: url('./img/delete.svg');
          }
        }
      }
    }
  }

  .error-info-dialog {
    width: 400px;
    height: 300px;
    position: fixed;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    box-shadow: 10px 10px 5px rgba($color: #575757, $alpha: 0.3);
    border: 1px solid rgba($color: #7f7f7f, $alpha: 0.3);
    padding-bottom: 14px;

    .info {
      width: 100%;
      height: 100%;
      font-size: 12px;
      overflow-y: auto;
    }

    .delete-operation {
      position: absolute;
      bottom: 0;
      left: 0;
      width: 100%;
      height: 14px;
      display: flex;
      justify-content: flex-end;

      .delete-btn {
        font-size: 12px;

        &:hover {
          color: #4872ac;
        }
      }
    }
  }

  .delete-dialog {
    width: 280px;
    position: absolute;
    left: calc(100% - 300px);
    bottom: 20px;
    // top: 50%;
    // transform: translateY(-50%);
    box-shadow: 10px 10px 5px rgba($color: #575757, $alpha: 0.3);
    border: 1px solid rgba($color: #7f7f7f, $alpha: 0.3);

    .delete-title {
      font-size: 12px;
      font-weight: bold;
    }

    .delete-item {
      font-size: 12px;
      font-weight: bold;
      color: rgb(130, 58, 79);
    }

    .delete-operation {
      display: flex;
      justify-content: space-between;

      .delete-btn {
        font-size: 12px;

        &:hover {
          color: #4872ac;
        }
      }
    }
  }

  .custom-menu {
    position: absolute;
    background-color: #f9f9f9;
    border: 1px solid #ccc;
    padding: 5px;
    box-shadow: 0 0 5px rgba(0, 0, 0, 0.3);
    z-index: 1000;
  }

  .custom-menu ul {
    list-style-type: none;
    padding: 0;
    margin: 0;
  }

  .custom-menu ul li {
    font-size: 12px;
    padding: 5px 5px;
    cursor: pointer;
  }

  .custom-menu ul li:hover {
    background-color: #f0f0f0;
  }
}
</style>
