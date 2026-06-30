use std::{ffi::CString, os::raw::c_void};

use ash::{Entry, vk, vk::Handle};
use skia_safe::gpu::vk as skia_vk;

use crate::SvgRenderError;

pub(crate) struct VulkanState {
    entry: Entry,
    pub(crate) instance: ash::Instance,
    pub(crate) physical_device: vk::PhysicalDevice,
    pub(crate) device: ash::Device,
    pub(crate) queue: vk::Queue,
    pub(crate) queue_family_index: u32,
}

impl VulkanState {
    pub(crate) fn new() -> Result<Self, SvgRenderError> {
        let entry = load_vulkan_entry()?;
        let app_name = CString::new("svg-renderer").expect("static app name has no nul byte");
        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(1)
            .engine_name(&app_name)
            .engine_version(1)
            .api_version(vk::make_api_version(0, 1, 1, 0));
        let instance_info = vk::InstanceCreateInfo::default().application_info(&app_info);
        let instance = unsafe { entry.create_instance(&instance_info, None)? };

        let (physical_device, queue_family_index) = unsafe {
            instance
                .enumerate_physical_devices()?
                .into_iter()
                .find_map(|physical_device| {
                    instance
                        .get_physical_device_queue_family_properties(physical_device)
                        .into_iter()
                        .enumerate()
                        .find(|(_, properties)| {
                            properties.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                        })
                        .map(|(index, _)| (physical_device, index as u32))
                })
                .ok_or(SvgRenderError::NoVulkanDevice)?
        };

        let queue_priorities = [1.0f32];
        let queue_info = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&queue_priorities)];
        let device_info = vk::DeviceCreateInfo::default().queue_create_infos(&queue_info);
        let device = unsafe { instance.create_device(physical_device, &device_info, None)? };
        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        Ok(Self {
            entry,
            instance,
            physical_device,
            device,
            queue,
            queue_family_index,
        })
    }

    pub(crate) fn get_proc(&self, proc: skia_vk::GetProcOf) -> skia_vk::GetProcResult {
        match proc {
            skia_vk::GetProcOf::Instance(instance, name) => unsafe {
                let instance = vk::Instance::from_raw(instance as u64);
                self.entry
                    .get_instance_proc_addr(instance, name)
                    .map_or(std::ptr::null(), |proc| proc as *const c_void)
            },
            skia_vk::GetProcOf::Device(device, name) => unsafe {
                let device = vk::Device::from_raw(device as u64);
                self.instance
                    .get_device_proc_addr(device, name)
                    .map_or(std::ptr::null(), |proc| proc as *const c_void)
            },
        }
    }
}

impl Drop for VulkanState {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

fn load_vulkan_entry() -> Result<Entry, SvgRenderError> {
    match unsafe { Entry::load() } {
        Ok(entry) => Ok(entry),
        Err(default_error) => {
            load_vulkan_entry_from_platform_names().map_err(|_| default_error.into())
        }
    }
}

fn load_vulkan_entry_from_platform_names() -> Result<Entry, ash::LoadingError> {
    for library_name in VULKAN_LIBRARY_NAMES {
        if let Ok(entry) = unsafe { Entry::load_from(library_name) } {
            return Ok(entry);
        }
    }

    unsafe { Entry::load() }
}

#[cfg(target_os = "windows")]
const VULKAN_LIBRARY_NAMES: &[&str] = &["vulkan-1.dll"];

#[cfg(target_os = "linux")]
const VULKAN_LIBRARY_NAMES: &[&str] = &["libvulkan.so.1", "libvulkan.so"];

#[cfg(target_os = "android")]
const VULKAN_LIBRARY_NAMES: &[&str] = &["libvulkan.so"];

#[cfg(any(target_os = "macos", target_os = "ios"))]
const VULKAN_LIBRARY_NAMES: &[&str] = &[
    "libvulkan.1.dylib",
    "libvulkan.dylib",
    "libMoltenVK.dylib",
    "MoltenVK.framework/MoltenVK",
];

#[cfg(all(
    not(target_os = "windows"),
    not(target_os = "linux"),
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
const VULKAN_LIBRARY_NAMES: &[&str] = &[];
