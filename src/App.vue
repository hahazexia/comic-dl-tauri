<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from '@tauri-apps/api/window';

async function add() {
  await invoke("add");
}

onMounted(() => {
  const appWindow = getCurrentWindow();
  document.querySelector('.menu')?.addEventListener('mousedown', (e: any) => {
    if (e.buttons === 1) {
      e.detail === 2
        ? appWindow.toggleMaximize() // Maximize on double click
        : appWindow.startDragging(); // Else start dragging
    }
  });
});
</script>

<template>
  <div class="main">
    <div class="menu">
      <div class="menu-option all active">All Tasks<span class="num" v-text="0"></span></div>
      <div class="menu-option downloading">Downloading<span class="num" v-text="0"></span></div>
      <div class="menu-option finished">Finished<span class="num" v-text="0"></span></div>
      <div class="menu-option stopped">Stopped<span class="num" v-text="0"></span></div>
      <div class="menu-option failed">Failed<span class="num" v-text="0"></span></div>
      <div class="add" title="create new task" @click="add"></div>
    </div>
    <div class="list">
      <div class="list-item">
        <div class="name" v-text="'下载项目'" :title="'下载项目'"></div>
        <div class="desc">
          <div class="status downloading" v-text="'Downloading'"></div>
          <div class="progress-num" v-text="'6.55%'"></div>
          <div class="info" v-text="'额外信息'"></div>
        </div>
        <div class="progress">
          <div class="progress-inner" :style="{
            width: '6.55%',
          }"></div>
        </div>
        <div class="tool-bar">
          <div class="left"></div>
          <div class="right">
            <div class="pause"></div>
            <div class="delete"></div>
          </div>
        </div>
      </div>
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
          .status, .progress-num, .info {
            font-size: 12px;
          }
          .progress-num {
            margin-left: 5px;
            color: #4872ac;
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
            .pause, .delete {
              background-repeat: no-repeat;
              background-size: contain;
              width: 20px;
              height: 20px;
              cursor: pointer;
            }
            .pause {
              background-image: url('./img/pause.png');
            }
            .delete {
              background-image: url('./img/delete.png');
            }
          }
        }
      }
    }
  }
</style>
