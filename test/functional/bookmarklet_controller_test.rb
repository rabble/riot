require File.expand_path(File.dirname(__FILE__) + "/../test_helper")

class BookmarkletControllerTest < ActionController::TestCase
  before :all do
    #@calendar = Calendar.make 
  end

  #Delete this example and add some real ones
  it "should use BookmarkletController" do
    controller.should be_an_instance_of(BookmarkletController)
  end

end
