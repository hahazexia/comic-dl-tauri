<script setup lang="ts">
import { ref, reactive, onMounted, computed } from "vue";
import { listen } from '@tauri-apps/api/event';
import { invoke, Channel } from "@tauri-apps/api/core";
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
const isModalOpen = ref(false);

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

async function add() {
  await invoke("add");
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
  }
}

function deleteTask(data: any) {
  del_task.value = data;
  openModal();
}

const openModal = () => {
  isModalOpen.value = true;
};

const closeModal = () => {
  isModalOpen.value = false;
};

const startOrPause = async (data: any, status: string) => {
  console.log(data);
  const onEvent = new Channel<DownloadEvent>();
  onEvent.onmessage = (message: any) => {
    const index = tasks_all.findIndex(item => item.id === message.id);
    const index2 = tasks_current.findIndex(item => item.id === message.id);

    tasks_all[index].progress = message.progress;
    tasks_all[index].now_count = message.now_count;
    tasks_all[index].error_vec = message.error_vec;

    tasks_current[index2].progress = message.progress;
    tasks_current[index2].now_count = message.now_count;
    tasks_current[index2].error_vec = message.error_vec;
  };
  await invoke("start_or_pause", { id: data.id, status: status, onEvent });
};

const confirmDelete = async () => {
  let del_id = await invoke("delete_tasks", { id: del_task.value?.id });
  const index = tasks_all.findIndex(item => item.id === del_id);
  const index2 = tasks_current.findIndex(item => item.id === del_id);
  if (index !== -1) {
    tasks_all.splice(index, 1);
    tasks_current.splice(index2, 1);
  }

  isModalOpen.value = false;
};

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
    tasks_all.sort((a: any) => {
      return a.status === 'downloading' ? -1 : 1;
    });
    tasks_current.sort((a: any) => {
      return a.status === 'downloading' ? -1 : 1;
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
    tasks_all.push(...res);
    tasks_current.push(...res);
  })();
});
</script>

<template>
  <div class="main">
    <div class="menu">
      <div class="menu-option all" :class="{ active: active_menu === 'all' }" @click="() => switchMenu('all')">All
        Tasks<span class="num" v-text="tasks_all.length"></span></div>
      <div class="menu-option downloading" :class="{ active: active_menu === 'downloading' }"
        @click="() => switchMenu('downloading')">Downloading<span class="num" v-text="task_downloading.length"></span>
      </div>
      <div class="menu-option finished" :class="{ active: active_menu === 'finished' }"
        @click="() => switchMenu('finished')">Finished<span class="num" v-text="task_finished.length"></span></div>
      <div class="menu-option stopped" :class="{ active: active_menu === 'stopped' }"
        @click="() => switchMenu('stopped')">Stopped<span class="num" v-text="task_stopped.length"></span></div>
      <div class="menu-option failed" :class="{ active: active_menu === 'failed' }" @click="() =>
        switchMenu('failed')">Failed<span class="num" v-text="task_failed.length"></span></div>
      <div class="add" title="create new task" @click="add"></div>
    </div>
    <div class="list">
      <div class="list-item" v-for="data in tasks_current" :key="data.id">
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
            <div :class="{
              pause: data.status === 'downloading',
              start: data.status !== 'downloading',
            }" @click="() => startOrPause(data, data.status === 'downloading' ? 'stopped' : 'downloading')"></div>
            <div class="delete" @click="() => deleteTask(data)"></div>
          </div>
        </div>
      </div>
    </div>
    <dialog class="delete-dialog" :open="isModalOpen" @close="isModalOpen = false">
      <div class="delete-title" v-text="`Confirm delete this task?`"></div>
      <p class="delete-item" v-text="`${del_task?.comic_name} ${del_task?.dl_type}`"></p>
      <div class="delete-operation">
        <button class="delete-btn" @click="confirmDelete">confirm</button>
        <button class="delete-btn" @click="closeModal">cancel</button>
      </div>
    </dialog>
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
      background-image: url('./img/add.png');
      background-repeat: no-repeat;
      background-size: contain;
      width: 20px;
      height: 20px;
      position: absolute;
      left: 10px;
      bottom: 10px;
    }

    .menu-option {
      display: flex;
      justify-content: space-between;
      align-items: center;
      position: relative;
      cursor: pointer;
      font-size: 16px;
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

      &.active {
        background-color: #F5F5F5;
      }

      &:hover {
        background-color: #F5F5F5;
      }

      .num {
        font-size: 12px;
        color: #5D5D5D;
      }
    }

    .all::before {
      background-image: url('./img/all.png');
    }

    .downloading::before {
      background-image: url('./img/download.png');
    }

    .finished::before {
      background-image: url('./img/ok.png');
    }

    .stopped::before {
      background-image: url('./img/stop.png');
    }

    .failed::before {
      background-image: url('./img/failed.png');
    }
  }

  .list {
    max-height: 100%;
    overflow-y: auto;
    flex: 1;

    .list-item {
      cursor: pointer;
      padding: 20px 10px;

      &:hover {
        background-color: #F5F5F5;
      }

      .name {
        font-size: 16px;
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
          background-image: url('./img/download.png');
        }

        .finished::before {
          background-image: url('./img/ok.png');
        }

        .stopped::before {
          background-image: url('./img/stop.png');
        }

        .failed::before {
          background-image: url('./img/failed.png');
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
        width: 95%;
        height: 3px;
        background-color: #E6E6E6;
        margin: 5px 0;

        .progress-inner {
          height: 3px;
          max-width: 100%;
          background-color: #4872ac;
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
          .delete {
            background-repeat: no-repeat;
            background-size: contain;
            width: 20px;
            height: 20px;
            cursor: pointer;
          }

          .pause {
            background-image: url('./img/pause.png');
          }

          .start {
            background-image: url('./img/start.png');
          }

          .delete {
            background-image: url('./img/delete.png');
          }
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
      font-size: 14px;
      font-weight: bold;
    }

    .delete-item {
      font-size: 12px;
      font-weight: bold;
      color: rgb(140, 65, 88);
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
}
</style>
