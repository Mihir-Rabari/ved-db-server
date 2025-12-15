import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { 
  Dialog, 
  DialogContent, 
  DialogDescription, 
  DialogFooter, 
  DialogHeader, 
  DialogTitle, 
  DialogTrigger 
} from '@/components/ui/dialog';
import { 
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { useConnectionStore } from '@/store';
import { useToast } from '@/hooks/use-toast';
import { 
  Plus, 
  Trash2, 
  Edit, 
  Key, 
  Users as UsersIcon, 
  Shield, 
  ShieldCheck, 
  Eye,
  Loader2,
  RefreshCw
} from 'lucide-react';

interface UserInfo {
  username: string;
  role: string;
  created_at: string;
  last_login: string | null;
  enabled: boolean;
}

interface UserFormData {
  username: string;
  password: string;
  confirmPassword: string;
  role: string;
}

interface PasswordChangeData {
  currentPassword: string;
  newPassword: string;
  confirmPassword: string;
}

export function UserManagement() {
  const { activeConnection } = useConnectionStore();
  const { toast } = useToast();
  
  const [users, setUsers] = useState<UserInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  
  // Create user dialog state
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [createUserData, setCreateUserData] = useState<UserFormData>({
    username: '',
    password: '',
    confirmPassword: '',
    role: 'read-only'
  });
  const [creatingUser, setCreatingUser] = useState(false);
  
  // Edit role dialog state
  const [editRoleDialogOpen, setEditRoleDialogOpen] = useState(false);
  const [editingUser, setEditingUser] = useState<UserInfo | null>(null);
  const [newRole, setNewRole] = useState('');
  const [updatingRole, setUpdatingRole] = useState(false);
  
  // Change password dialog state
  const [passwordDialogOpen, setPasswordDialogOpen] = useState(false);
  const [passwordChangeUser, setPasswordChangeUser] = useState<UserInfo | null>(null);
  const [passwordData, setPasswordData] = useState<PasswordChangeData>({
    currentPassword: '',
    newPassword: '',
    confirmPassword: ''
  });
  
  // Delete user dialog state
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [userToDelete, setUserToDelete] = useState<UserInfo | null>(null);
  const [deletingUser, setDeletingUser] = useState(false);

  const roles = [
    { value: 'admin', label: 'Admin', description: 'Full access to all operations' },
    { value: 'read-write', label: 'Read-Write', description: 'Read and write operations' },
    { value: 'read-only', label: 'Read-Only', description: 'Read operations only' }
  ];

  // Load users on component mount and when connection changes
  useEffect(() => {
    if (activeConnection?.isConnected) {
      loadUsers();
    }
  }, [activeConnection]);

  const loadUsers = async () => {
    if (!activeConnection?.isConnected) return;
    
    setLoading(true);
    try {
      const userList = await invoke<UserInfo[]>('get_users', {
        connectionId: activeConnection.id
      });
      setUsers(userList);
    } catch (error) {
      console.error('Failed to load users:', error);
      toast({
        title: "Failed to Load Users",
        description: `Error: ${error}`,
        variant: "destructive",
      });
    } finally {
      setLoading(false);
    }
  };

  const handleRefresh = async () => {
    setRefreshing(true);
    await loadUsers();
    setRefreshing(false);
  };

  const handleCreateUser = async () => {
    if (!activeConnection?.isConnected) return;
    
    // Validate form
    if (!createUserData.username.trim()) {
      toast({
        title: "Validation Error",
        description: "Username is required",
        variant: "destructive",
      });
      return;
    }
    
    if (!createUserData.password) {
      toast({
        title: "Validation Error", 
        description: "Password is required",
        variant: "destructive",
      });
      return;
    }
    
    if (createUserData.password !== createUserData.confirmPassword) {
      toast({
        title: "Validation Error",
        description: "Passwords do not match",
        variant: "destructive",
      });
      return;
    }
    
    setCreatingUser(true);
    try {
      await invoke('create_user', {
        connectionId: activeConnection.id,
        username: createUserData.username.trim(),
        password: createUserData.password,
        role: createUserData.role
      });
      
      toast({
        title: "User Created",
        description: `User "${createUserData.username}" has been created successfully.`,
        variant: "default",
      });
      
      // Reset form and close dialog
      setCreateUserData({
        username: '',
        password: '',
        confirmPassword: '',
        role: 'read-only'
      });
      setCreateDialogOpen(false);
      
      // Reload users
      await loadUsers();
    } catch (error) {
      console.error('Failed to create user:', error);
      toast({
        title: "Failed to Create User",
        description: `Error: ${error}`,
        variant: "destructive",
      });
    } finally {
      setCreatingUser(false);
    }
  };

  const handleDeleteUser = async () => {
    if (!activeConnection?.isConnected || !userToDelete) return;
    
    setDeletingUser(true);
    try {
      await invoke('delete_user', {
        connectionId: activeConnection.id,
        username: userToDelete.username
      });
      
      toast({
        title: "User Deleted",
        description: `User "${userToDelete.username}" has been deleted successfully.`,
        variant: "default",
      });
      
      setDeleteDialogOpen(false);
      setUserToDelete(null);
      
      // Reload users
      await loadUsers();
    } catch (error) {
      console.error('Failed to delete user:', error);
      toast({
        title: "Failed to Delete User",
        description: `Error: ${error}`,
        variant: "destructive",
      });
    } finally {
      setDeletingUser(false);
    }
  };

  const handleUpdateRole = async () => {
    if (!activeConnection?.isConnected || !editingUser) return;
    
    setUpdatingRole(true);
    try {
      await invoke('update_user_role', {
        connectionId: activeConnection.id,
        username: editingUser.username,
        role: newRole
      });
      
      toast({
        title: "Role Updated",
        description: `Role for "${editingUser.username}" has been updated to ${newRole}.`,
        variant: "default",
      });
      
      setEditRoleDialogOpen(false);
      setEditingUser(null);
      setNewRole('');
      
      // Reload users
      await loadUsers();
    } catch (error) {
      console.error('Failed to update user role:', error);
      toast({
        title: "Failed to Update Role",
        description: `Error: ${error}`,
        variant: "destructive",
      });
    } finally {
      setUpdatingRole(false);
    }
  };

  const openEditRoleDialog = (user: UserInfo) => {
    setEditingUser(user);
    setNewRole(user.role);
    setEditRoleDialogOpen(true);
  };

  const openDeleteDialog = (user: UserInfo) => {
    setUserToDelete(user);
    setDeleteDialogOpen(true);
  };

  const openPasswordDialog = (user: UserInfo) => {
    setPasswordChangeUser(user);
    setPasswordData({
      currentPassword: '',
      newPassword: '',
      confirmPassword: ''
    });
    setPasswordDialogOpen(true);
  };

  const getRoleIcon = (role: string) => {
    switch (role) {
      case 'admin':
        return <ShieldCheck className="h-4 w-4" />;
      case 'read-write':
        return <Shield className="h-4 w-4" />;
      case 'read-only':
        return <Eye className="h-4 w-4" />;
      default:
        return <Shield className="h-4 w-4" />;
    }
  };

  const getRoleColor = (role: string) => {
    switch (role) {
      case 'admin':
        return 'bg-red-100 text-red-800 border-red-200';
      case 'read-write':
        return 'bg-blue-100 text-blue-800 border-blue-200';
      case 'read-only':
        return 'bg-green-100 text-green-800 border-green-200';
      default:
        return 'bg-gray-100 text-gray-800 border-gray-200';
    }
  };

  const formatDate = (dateString: string) => {
    return new Date(dateString).toLocaleString();
  };

  if (!activeConnection?.isConnected) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <UsersIcon className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h3 className="text-lg font-medium mb-2">No Connection</h3>
          <p className="text-muted-foreground">
            Connect to a VedDB server to manage users.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col">
      {/* Header */}
      <div className="border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="flex h-14 items-center px-6">
          <div className="flex items-center gap-2">
            <UsersIcon className="h-5 w-5" />
            <h1 className="font-semibold">User Management</h1>
          </div>
          <div className="ml-auto flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={handleRefresh}
              disabled={refreshing}
            >
              {refreshing ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <RefreshCw className="h-4 w-4" />
              )}
              Refresh
            </Button>
            <Dialog open={createDialogOpen} onOpenChange={setCreateDialogOpen}>
              <DialogTrigger asChild>
                <Button size="sm">
                  <Plus className="h-4 w-4 mr-2" />
                  Create User
                </Button>
              </DialogTrigger>
              <DialogContent>
                <DialogHeader>
                  <DialogTitle>Create New User</DialogTitle>
                  <DialogDescription>
                    Create a new user account with specified role and permissions.
                  </DialogDescription>
                </DialogHeader>
                <div className="space-y-4">
                  <div className="space-y-2">
                    <Label htmlFor="create-username">Username</Label>
                    <Input
                      id="create-username"
                      placeholder="Enter username"
                      value={createUserData.username}
                      onChange={(e) => setCreateUserData(prev => ({
                        ...prev,
                        username: e.target.value
                      }))}
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="create-password">Password</Label>
                    <Input
                      id="create-password"
                      type="password"
                      placeholder="Enter password"
                      value={createUserData.password}
                      onChange={(e) => setCreateUserData(prev => ({
                        ...prev,
                        password: e.target.value
                      }))}
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="create-confirm-password">Confirm Password</Label>
                    <Input
                      id="create-confirm-password"
                      type="password"
                      placeholder="Confirm password"
                      value={createUserData.confirmPassword}
                      onChange={(e) => setCreateUserData(prev => ({
                        ...prev,
                        confirmPassword: e.target.value
                      }))}
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="create-role">Role</Label>
                    <Select
                      value={createUserData.role}
                      onValueChange={(value) => setCreateUserData(prev => ({
                        ...prev,
                        role: value
                      }))}
                    >
                      <SelectTrigger>
                        <SelectValue placeholder="Select a role" />
                      </SelectTrigger>
                      <SelectContent>
                        {roles.map((role) => (
                          <SelectItem key={role.value} value={role.value}>
                            <div className="flex items-center gap-2">
                              {getRoleIcon(role.value)}
                              <div>
                                <div className="font-medium">{role.label}</div>
                                <div className="text-xs text-muted-foreground">{role.description}</div>
                              </div>
                            </div>
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                </div>
                <DialogFooter>
                  <Button
                    variant="outline"
                    onClick={() => setCreateDialogOpen(false)}
                  >
                    Cancel
                  </Button>
                  <Button
                    onClick={handleCreateUser}
                    disabled={creatingUser}
                  >
                    {creatingUser ? (
                      <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                    ) : null}
                    Create User
                  </Button>
                </DialogFooter>
              </DialogContent>
            </Dialog>
          </div>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 p-6">
        {loading ? (
          <div className="flex items-center justify-center h-64">
            <Loader2 className="h-8 w-8 animate-spin" />
          </div>
        ) : (
          <div className="grid gap-4">
            {users.length === 0 ? (
              <Card>
                <CardContent className="flex items-center justify-center h-64">
                  <div className="text-center">
                    <UsersIcon className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
                    <h3 className="text-lg font-medium mb-2">No Users Found</h3>
                    <p className="text-muted-foreground mb-4">
                      No users are currently configured on this server.
                    </p>
                    <Button onClick={() => setCreateDialogOpen(true)}>
                      <Plus className="h-4 w-4 mr-2" />
                      Create First User
                    </Button>
                  </div>
                </CardContent>
              </Card>
            ) : (
              users.map((user) => (
                <Card key={user.username}>
                  <CardHeader>
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-3">
                        <div className="h-10 w-10 rounded-full bg-muted flex items-center justify-center">
                          <UsersIcon className="h-5 w-5" />
                        </div>
                        <div>
                          <CardTitle className="text-lg">{user.username}</CardTitle>
                          <CardDescription>
                            Created {formatDate(user.created_at)}
                          </CardDescription>
                        </div>
                      </div>
                      <div className="flex items-center gap-2">
                        <Badge 
                          variant="outline" 
                          className={getRoleColor(user.role)}
                        >
                          {getRoleIcon(user.role)}
                          <span className="ml-1 capitalize">{user.role}</span>
                        </Badge>
                        <Badge variant={user.enabled ? "default" : "secondary"}>
                          {user.enabled ? "Enabled" : "Disabled"}
                        </Badge>
                      </div>
                    </div>
                  </CardHeader>
                  <CardContent>
                    <div className="flex items-center justify-between">
                      <div className="text-sm text-muted-foreground">
                        Last login: {user.last_login ? formatDate(user.last_login) : 'Never'}
                      </div>
                      <div className="flex items-center gap-2">
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => openEditRoleDialog(user)}
                        >
                          <Edit className="h-4 w-4 mr-2" />
                          Edit Role
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => openPasswordDialog(user)}
                        >
                          <Key className="h-4 w-4 mr-2" />
                          Change Password
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => openDeleteDialog(user)}
                          disabled={user.username === 'admin'} // Prevent deleting admin user
                        >
                          <Trash2 className="h-4 w-4 mr-2" />
                          Delete
                        </Button>
                      </div>
                    </div>
                  </CardContent>
                </Card>
              ))
            )}
          </div>
        )}
      </div>

      {/* Edit Role Dialog */}
      <Dialog open={editRoleDialogOpen} onOpenChange={setEditRoleDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Edit User Role</DialogTitle>
            <DialogDescription>
              Change the role for user "{editingUser?.username}".
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="edit-role">Role</Label>
              <Select value={newRole} onValueChange={setNewRole}>
                <SelectTrigger>
                  <SelectValue placeholder="Select a role" />
                </SelectTrigger>
                <SelectContent>
                  {roles.map((role) => (
                    <SelectItem key={role.value} value={role.value}>
                      <div className="flex items-center gap-2">
                        {getRoleIcon(role.value)}
                        <div>
                          <div className="font-medium">{role.label}</div>
                          <div className="text-xs text-muted-foreground">{role.description}</div>
                        </div>
                      </div>
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setEditRoleDialogOpen(false)}
            >
              Cancel
            </Button>
            <Button
              onClick={handleUpdateRole}
              disabled={updatingRole || newRole === editingUser?.role}
            >
              {updatingRole ? (
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
              ) : null}
              Update Role
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Change Password Dialog */}
      <Dialog open={passwordDialogOpen} onOpenChange={setPasswordDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Change Password</DialogTitle>
            <DialogDescription>
              Change the password for user "{passwordChangeUser?.username}".
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="current-password">Current Password</Label>
              <Input
                id="current-password"
                type="password"
                placeholder="Enter current password"
                value={passwordData.currentPassword}
                onChange={(e) => setPasswordData(prev => ({
                  ...prev,
                  currentPassword: e.target.value
                }))}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="new-password">New Password</Label>
              <Input
                id="new-password"
                type="password"
                placeholder="Enter new password"
                value={passwordData.newPassword}
                onChange={(e) => setPasswordData(prev => ({
                  ...prev,
                  newPassword: e.target.value
                }))}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="confirm-new-password">Confirm New Password</Label>
              <Input
                id="confirm-new-password"
                type="password"
                placeholder="Confirm new password"
                value={passwordData.confirmPassword}
                onChange={(e) => setPasswordData(prev => ({
                  ...prev,
                  confirmPassword: e.target.value
                }))}
              />
            </div>
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setPasswordDialogOpen(false)}
            >
              Cancel
            </Button>
            <Button
              onClick={() => {
                // Password change functionality would be implemented here
                // For now, just show a toast that it's not implemented
                toast({
                  title: "Feature Not Implemented",
                  description: "Password change functionality will be implemented when the server supports it.",
                  variant: "default",
                });
                setPasswordDialogOpen(false);
              }}
              disabled={false}
            >
              Change Password
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete User Dialog */}
      <Dialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete User</DialogTitle>
            <DialogDescription>
              Are you sure you want to delete user "{userToDelete?.username}"? This action cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setDeleteDialogOpen(false)}
            >
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={handleDeleteUser}
              disabled={deletingUser}
            >
              {deletingUser ? (
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
              ) : null}
              Delete User
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}