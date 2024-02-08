## bugs found in Ros-Rolling

| #  | Scope/Module         | Bug Type               | Operations                                         |
|----|----------------------|------------------------|----------------------------------------------------|
| 1  | Runtime/rmw_fastrtps | Data-race              | ~Condition / wait                                  |
| 2  | Runtime/eProsima     | Data-race              | get_listener_for / delete_datawriter               |
| 3  | Runtime/eProsima     | Data-race              | set_status / get_subscription_matched_status       |
| 4  | Runtime/Rclcpp       | Deadlock               | lifecycle_service_client                           |
| 5  | Runtime/Rclcpp       | Deadlock               | action_client                                      |
| 6  | Runtime/eProsima     | Data-race              | get_publication_matched_status / set_status        |
| 7  | Runtime/eProsima     | Data-race              | set_read_communication_status / set_status         |
| 8  | Runtime/Rclcpp       | Deadlock               | double_unlock                                      |
| 9  | Runtime/eProsima     | Data-race              |  deliver_sample_nts / change_received              |
| 10 | Runtime/geometry2    | Data-race              | create_new_change / unsent_change_added_to_history |
| 11 | Runtime/eProsima     |  heap-use-after-free   | write                                              |
| 12 | Runtime/fastrtps     | alloc-dealloc-mismatch | new_allocator_impl                                 |
| 13 | Runtime/ROSIDL       | memory leaks           | get_typesupport_handle_function                    |
| 14 | Runtime/tlsf_cpp     | memory leaks           | initialize                                         |
| 15 | Runtime/tlsf_cpp     | memory leaks           | tlsf_heap_allocator                                |


## Bug count in 24H evaluation
| **Tools** | **Autoware** | **Turtlebot3** | **Navigator2** | **Turtlesim** | **Runtime** | **Sum** |
|:---------:|:------------:|:--------------:|:--------------:|:-------------:|:-----------:|:-------:|
| R2D2      | 7            | 6              | 2              | 0             | 9           | 24      |
| ROZZ      | 3            | 5              | 4              | 0             | 2           | 12      |
| Robofuzz  | 0            | 9              | 0              | 2             | 5           | 16      |
| Ros2Fuzz  | 0            | 0              | 1              | 0             | 0           | 1       |
